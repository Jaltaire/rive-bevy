use std::{
    collections::HashMap,
    ffi::c_void,
    sync::atomic::Ordering,
};

use bevy::{
    prelude::*,
    render::{
        render_asset::RenderAssets, render_resource::TextureFormat, renderer::RenderQueue,
        texture::GpuImage,
    },
};
use rive_rs::metal_renderer::{
    self, Alignment, Fit, LoadAction, MetalRenderTarget,
};

use crate::metal::{MetalRenderContext, MetalSceneDraw};

#[derive(Default, Resource)]
pub(crate) struct MetalTargets {
    targets: HashMap<(u32, u32, TextureFormat), MetalRenderTarget>,
    has_rendered_this_frame: bool,
}

pub(crate) fn reset_targets(mut targets: ResMut<MetalTargets>) {
    targets.has_rendered_this_frame = false;
}

fn mtl_pixel_format(format: TextureFormat) -> u32 {
    const MTL_PIXEL_FORMAT_RGBA8_UNORM: u32 = 70;
    const MTL_PIXEL_FORMAT_RGBA8_UNORM_SRGB: u32 = 71;
    const MTL_PIXEL_FORMAT_BGRA8_UNORM: u32 = 80;
    const MTL_PIXEL_FORMAT_BGRA8_UNORM_SRGB: u32 = 81;

    match format {
        TextureFormat::Rgba8Unorm => MTL_PIXEL_FORMAT_RGBA8_UNORM,
        TextureFormat::Rgba8UnormSrgb => MTL_PIXEL_FORMAT_RGBA8_UNORM_SRGB,
        TextureFormat::Bgra8Unorm => MTL_PIXEL_FORMAT_BGRA8_UNORM,
        TextureFormat::Bgra8UnormSrgb => MTL_PIXEL_FORMAT_BGRA8_UNORM_SRGB,
        other => panic!(
            "The Rive Metal renderer cannot target images with format {other:?}. Use one of Rgba8Unorm, Rgba8UnormSrgb, Bgra8Unorm, or Bgra8UnormSrgb."
        ),
    }
}

pub(crate) fn render_metal_scene_textures(
    query: Query<&MetalSceneDraw>,
    gpu_images: Res<RenderAssets<GpuImage>>,
    render_queue: Res<RenderQueue>,
    context: Res<MetalRenderContext>,
    mut targets: ResMut<MetalTargets>,
) {
    if targets.has_rendered_this_frame {
        return;
    }
    targets.has_rendered_this_frame = true;

    if query.is_empty() {
        return;
    }

    let raw_queue = unsafe {
        render_queue
            .as_hal::<wgpu::hal::api::Metal>()
            .map(|hal_queue| hal_queue.as_raw() as *const _ as *mut c_void)
    }
    .expect(
        "The metal-renderer feature requires the Metal wgpu backend, but the render queue is not backed by Metal.",
    );

    let mut context = context.0.lock().unwrap();

    for scene in &query {
        let Some(gpu_image) = gpu_images.get(scene.image_handle.id()) else {
            debug!("The Rive output image is not yet available on the GPU. Retrying next frame...");
            continue;
        };

        let format = gpu_image.texture_descriptor.format;
        let width = scene.width;
        let height = scene.height;

        if gpu_image.texture_descriptor.size.width != width
            || gpu_image.texture_descriptor.size.height != height
        {
            warn!(
                "The Rive viewport is {}x{} but its target image is {}x{}. Skipping the draw until they match...",
                width,
                height,
                gpu_image.texture_descriptor.size.width,
                gpu_image.texture_descriptor.size.height,
            );
            continue;
        }

        let render_target = targets
            .targets
            .entry((width, height, format))
            .or_insert_with(|| context.make_render_target(mtl_pixel_format(format), width, height));

        let raw_texture = unsafe {
            gpu_image
                .texture
                .as_hal::<wgpu::hal::api::Metal>()
                .map(|hal_texture| hal_texture.raw_handle() as *const _ as *mut c_void)
        }
        .expect("The Rive output image is not backed by a Metal texture.");

        unsafe {
            render_target.set_target_texture(raw_texture);
        }

        let _artboard_guard = scene.handle.artboard_lock.lock().unwrap();

        context.begin_frame(width, height, LoadAction::Clear, 0);
        context.draw_artboard(&scene.handle.artboard, Fit::Contain, Alignment::CENTER, 1.0);

        unsafe {
            let command_buffer = metal_renderer::command_buffer_new(raw_queue);
            context.flush(render_target, command_buffer);
            metal_renderer::command_buffer_commit(command_buffer);
        }

        scene.handle.has_drawn.store(true, Ordering::Relaxed);
    }
}

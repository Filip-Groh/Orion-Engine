use super::*;

#[test]
fn fixed_surface_extent_takes_precedence_and_zero_extent_skips_creation() {
    let mut capabilities = vk::SurfaceCapabilitiesKHR::default();
    let requested = vk::Extent2D {
        width: 640,
        height: 480,
    };
    assert_eq!(choose_extent(&capabilities, requested), None);
    let fixed = vk::Extent2D {
        width: 800,
        height: 600,
    };
    capabilities.current_extent = fixed;
    assert_eq!(choose_extent(&capabilities, requested), Some(fixed));
}

#[test]
fn variable_surface_extent_clamps_each_axis() {
    let capabilities = vk::SurfaceCapabilitiesKHR::default()
        .current_extent(vk::Extent2D {
            width: u32::MAX,
            height: u32::MAX,
        })
        .min_image_extent(vk::Extent2D {
            width: 100,
            height: 200,
        })
        .max_image_extent(vk::Extent2D {
            width: 800,
            height: 600,
        });
    let requested = vk::Extent2D {
        width: 1_000,
        height: 100,
    };
    assert_eq!(
        choose_extent(&capabilities, requested),
        Some(vk::Extent2D {
            width: 800,
            height: 200
        })
    );
}

#[test]
fn present_mode_follows_preference_order_regardless_of_surface_order() {
    use vk::PresentModeKHR as Mode;
    assert_eq!(
        choose_present_mode(&[Mode::FIFO, Mode::MAILBOX, Mode::IMMEDIATE]),
        Mode::IMMEDIATE
    );
    assert_eq!(
        choose_present_mode(&[Mode::FIFO, Mode::MAILBOX]),
        Mode::MAILBOX
    );
    assert_eq!(
        choose_present_mode(&[Mode::FIFO_RELAXED, Mode::FIFO]),
        Mode::FIFO
    );
}

#[test]
fn format_selection_skips_incompatible_color_spaces_and_pixel_formats() {
    use vk::{ColorSpaceKHR as Space, Format};
    let unsupported_format = vk::SurfaceFormatKHR::default()
        .format(Format::R16G16B16A16_SFLOAT)
        .color_space(Space::SRGB_NONLINEAR);
    let unsupported_space = vk::SurfaceFormatKHR::default()
        .format(Format::R8G8B8A8_UNORM)
        .color_space(Space::DISPLAY_P3_NONLINEAR_EXT);
    let supported = vk::SurfaceFormatKHR::default()
        .format(Format::B8G8R8A8_SRGB)
        .color_space(Space::SRGB_NONLINEAR);
    assert!(choose_format(&[unsupported_format, unsupported_space]).is_none());
    let chosen = choose_format(&[unsupported_format, unsupported_space, supported]).unwrap();
    assert_eq!(chosen.format, supported.format);
    assert_eq!(chosen.color_space, supported.color_space);
}

#[test]
fn preferred_image_count_obeys_the_surface_maximum() {
    let capabilities = vk::SurfaceCapabilitiesKHR::default().min_image_count(2);
    assert_eq!(image_count(&capabilities), 3);
    assert_eq!(image_count(&capabilities.max_image_count(2)), 2);
}

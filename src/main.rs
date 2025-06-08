// #![windows_subsystem = "windows"]

use arboard::Clipboard;
use screenshots::Screen;
use slint::LogicalPosition;
use std::cell::RefCell;
use std::error::Error;
use std::rc::Rc;

// 导入UI组件
slint::include_modules!();

// 添加预览窗口状态结构体
struct PreviewWindowState {
    window: Rc<PreviewWindow>,
}

impl PreviewWindowState {
    fn new(image_data: Vec<u8>, width: u32, height: u32) -> Rc<RefCell<Self>> {
        let window = Rc::new(PreviewWindow::new().unwrap());

        // 创建slint图像
        let mut pixel_buffer = slint::SharedPixelBuffer::<slint::Rgba8Pixel>::new(width, height);
        let buffer = pixel_buffer.make_mut_bytes();

        // 直接复制数据
        buffer.copy_from_slice(&image_data);

        let slint_image = slint::Image::from_rgba8(pixel_buffer);
        window.set_screenshot(slint_image);

        let instance = Rc::new(RefCell::new(Self {
            window: window.clone(),
        }));

        let window_weak = window.as_weak();
        window.on_close_window(move || {
            if let Some(window) = window_weak.upgrade() {
                window.hide().unwrap();
            }
        });

        let window_weak = window.as_weak();
        window.on_move_window(move |offset_x, offset_y| {
            if let Some(window) = window_weak.upgrade() {
                let pos = window.window().position();
                let scale = window.window().scale_factor();
                let logical_pos = pos.to_logical(scale);
                window.window().set_position(slint::LogicalPosition::new(
                    logical_pos.x + offset_x,
                    logical_pos.y + offset_y,
                ));
            }
        });
        instance
    }

    fn show(&self) {
        self.window.show().unwrap();
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    // 直接显示截图窗口
    show_screenshot_window()?;
    Ok(())
}

fn show_screenshot_window() -> Result<(), Box<dyn Error>> {
    // 简化的屏幕截图
    if let Ok(screens) = Screen::all() {
        if let Some(screen) = screens.first() {
            if let Ok(image) = screen.capture() {
                let width = image.width();
                let height = image.height();
                let background_data = image.into_raw();

                let app = AppWindow::new()?;

                // 直接从二进制数据创建 Slint 图像
                let mut pixel_buffer =
                    slint::SharedPixelBuffer::<slint::Rgba8Pixel>::new(width, height);
                let buffer = pixel_buffer.make_mut_bytes();

                // 直接复制数据
                buffer.copy_from_slice(&background_data);
                app.window()
                    .set_position(LogicalPosition::new(0 as f32, 0 as f32));
                let background_image = slint::Image::from_rgba8(pixel_buffer);
                app.set_background_screenshot(background_image);

                // 在显示窗口之前设置遮罩
                app.set_show_mask(true);

                // 添加调试日志回调
                app.on_debug_log(move |message| {
                    println!("Debug: {}", message);
                });

                // 处理选区完成
                let app_weak = app.as_weak();
                let background_data_clone = background_data.clone();
                app.on_selection_complete(move |area| {
                    if let Some(app) = app_weak.upgrade() {
                        app.set_show_decorations(false);
                        app.hide().unwrap();

                        // 从背景数据中提取选区
                        let capture_x = area.x as u32;
                        let capture_y = area.y as u32;
                        let capture_width = area.width as u32;
                        let capture_height = area.height as u32;

                        if let Ok(()) = extract_selection_from_background(
                            &background_data_clone,
                            width,
                            height,
                            capture_x,
                            capture_y,
                            capture_width,
                            capture_height,
                            area.x as i32,
                            area.y as i32,
                        ) {
                            println!("截图完成");
                        } else {
                            println!("截图失败");
                        }
                    }
                });

                // 处理取消截图
                let app_weak = app.as_weak();
                app.on_cancel_capture(move || {
                    println!("取消截图");
                    if let Some(app) = app_weak.upgrade() {
                        app.hide().unwrap();
                        std::process::exit(0);
                    }
                });

                // 显示窗口并运行事件循环
                app.show()?;
                app.run()?;
            }
        }
    }

    Ok(())
}

// 直接从背景二进制数据提取选区
fn extract_selection_from_background(
    background_data: &[u8],
    bg_width: u32,
    bg_height: u32,
    sel_x: u32,
    sel_y: u32,
    sel_width: u32,
    sel_height: u32,
    window_x: i32,
    window_y: i32,
) -> Result<(), Box<dyn Error>> {
    if sel_width == 0 || sel_height == 0 {
        return Ok(());
    }

    // 直接创建选区的二进制数据
    let mut selection_data = vec![0u8; (sel_width * sel_height * 4) as usize];

    for y in 0..sel_height {
        for x in 0..sel_width {
            let src_x = sel_x + x;
            let src_y = sel_y + y;

            if src_x < bg_width && src_y < bg_height {
                let src_idx = ((src_y * bg_width + src_x) * 4) as usize;
                let dst_idx = ((y * sel_width + x) * 4) as usize;

                if src_idx + 3 < background_data.len() && dst_idx + 3 < selection_data.len() {
                    selection_data[dst_idx] = background_data[src_idx]; // R
                    selection_data[dst_idx + 1] = background_data[src_idx + 1]; // G
                    selection_data[dst_idx + 2] = background_data[src_idx + 2]; // B
                    selection_data[dst_idx + 3] = background_data[src_idx + 3]; // A
                }
            }
        }
    }

    // 复制到剪贴板
    let mut clipboard = Clipboard::new()?;
    clipboard.set_image(arboard::ImageData {
        width: sel_width as usize,
        height: sel_height as usize,
        bytes: selection_data.clone().into(),
    })?;

    // 创建预览窗口
    let preview = PreviewWindowState::new(selection_data, sel_width, sel_height);
    preview
        .borrow()
        .window
        .window()
        .set_position(slint::LogicalPosition::new(
            window_x as f32,
            window_y as f32,
        ));
    preview.borrow().show();

    Ok(())
}

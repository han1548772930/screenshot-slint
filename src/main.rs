#![windows_subsystem = "windows"]

use arboard::Clipboard;
use screenshots::Screen;
use slint::{LogicalPosition, ModelRc, VecModel};
use std::cell::RefCell;
use std::error::Error;
use std::rc::Rc;

// 导入UI组件
slint::include_modules!();

// 添加选区状态结构体
#[derive(Debug, Clone)]
struct SelectionState {
    start_x: f32,
    start_y: f32,
    current_x: f32,
    current_y: f32,
    is_selecting: bool,
    is_dragging: bool,
    is_resizing: bool,
    is_drawing: bool, // 添加这个字段来区分是否正在绘制
    drag_offset_x: f32,
    drag_offset_y: f32,
    resize_mode: String,
    current_handle: String,
}

impl Default for SelectionState {
    fn default() -> Self {
        Self {
            start_x: 0.0,
            start_y: 0.0,
            current_x: 0.0,
            current_y: 0.0,
            is_selecting: false,
            is_dragging: false,
            is_resizing: false,
            is_drawing: false, // 新增字段
            drag_offset_x: 0.0,
            drag_offset_y: 0.0,
            resize_mode: String::new(),
            current_handle: String::new(),
        }
    }
}

#[derive(Debug, Clone)]
struct RustRectangleObject {
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    start_x: f32,
    start_y: f32,
    end_x: f32,
    end_y: f32,
    selected: bool,
}
#[derive(Debug, Clone)]
struct RustPenPoint {
    x: f32,
    y: f32,
}

#[derive(Debug, Clone)]
struct RustPenPath {
    points: Vec<RustPenPoint>,
    color: String,
    width: f32,
    // 添加预计算的边界框
    min_x: f32,
    max_x: f32,
    min_y: f32,
    max_y: f32,
    // 添加预计算的 SVG 命令
    commands: String,
    // 添加缓存标志
    commands_cached: bool,
}
#[derive(Debug, Clone)]
struct RustArrowObject {
    start_x: f32,
    start_y: f32,
    end_x: f32,
    end_y: f32,
    selected: bool,
}

#[derive(Debug, Clone)]
struct RustCircleObject {
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    start_x: f32,
    start_y: f32,
    end_x: f32,
    end_y: f32,
    selected: bool,
}
impl RustPenPath {
    fn new(points: Vec<RustPenPoint>, color: String, width: f32) -> Self {
        let (min_x, max_x, min_y, max_y) = if points.is_empty() {
            (0.0, 0.0, 0.0, 0.0)
        } else {
            let mut min_x = points[0].x;
            let mut max_x = points[0].x;
            let mut min_y = points[0].y;
            let mut max_y = points[0].y;

            for point in &points {
                min_x = min_x.min(point.x);
                max_x = max_x.max(point.x);
                min_y = min_y.min(point.y);
                max_y = max_y.max(point.y);
            }

            (min_x, max_x, min_y, max_y)
        };

        // 生成相对坐标的 SVG 命令
        let commands = if points.is_empty() {
            String::new()
        } else {
            let mut cmd = format!(
                "M {} {}",
                points[0].x - min_x + 10.0,
                points[0].y - min_y + 10.0
            );
            for point in points.iter().skip(1) {
                cmd.push_str(&format!(
                    " L {} {}",
                    point.x - min_x + 10.0,
                    point.y - min_y + 10.0
                ));
            }
            cmd
        };

        Self {
            points,
            color,
            width,
            min_x,
            max_x,
            min_y,
            max_y,
            commands,
            commands_cached: true,
        }
    }
    fn recalculate_bounds_and_commands(&mut self) {
        if self.commands_cached {
            return; // 如果已经缓存，不需要重新计算
        }

        let (min_x, max_x, min_y, max_y) = if self.points.is_empty() {
            (0.0, 0.0, 0.0, 0.0)
        } else {
            let mut min_x = self.points[0].x;
            let mut max_x = self.points[0].x;
            let mut min_y = self.points[0].y;
            let mut max_y = self.points[0].y;

            for point in &self.points {
                min_x = min_x.min(point.x);
                max_x = max_x.max(point.x);
                min_y = min_y.min(point.y);
                max_y = max_y.max(point.y);
            }

            (min_x, max_x, min_y, max_y)
        };

        // 生成相对坐标的 SVG 命令
        self.commands = if self.points.is_empty() {
            String::new()
        } else {
            let mut cmd = format!(
                "M {} {}",
                self.points[0].x - min_x + 10.0,
                self.points[0].y - min_y + 10.0
            );
            for point in self.points.iter().skip(1) {
                cmd.push_str(&format!(
                    " L {} {}",
                    point.x - min_x + 10.0,
                    point.y - min_y + 10.0
                ));
            }
            cmd
        };

        self.min_x = min_x;
        self.max_x = max_x;
        self.min_y = min_y;
        self.max_y = max_y;
        self.commands_cached = true;
    }

    // 添加点时标记缓存失效
    fn add_point(&mut self, point: RustPenPoint) {
        self.points.push(point);
        self.commands_cached = false; // 标记缓存失效
    }

    // 获取命令字符串（自动处理缓存）
    fn get_commands(&mut self) -> &str {
        if !self.commands_cached {
            self.recalculate_bounds_and_commands();
        }
        &self.commands
    }
}
// 应用状态结构体
struct AppState {
    selection: SelectionState,
    // 分开处理矩形、圆形和箭头对象
    rectangle_objects: Vec<RustRectangleObject>,
    circle_objects: Vec<RustCircleObject>,
    arrow_objects: Vec<RustArrowObject>, // 添加这一行
    selected_object_index: i32,
    drawing_mode: String,
    is_drawing: bool,
    selected_icon: String,
    handle_size: f32,
    // 新增：画框模式相关
    is_drawing_mode: bool,
    // 分开处理当前绘制对象
    current_rectangle_object: Option<RustRectangleObject>,
    current_circle_object: Option<RustCircleObject>,
    current_arrow_object: Option<RustArrowObject>, // 添加这一行
    is_drawing_rectangle: bool,
    is_drawing_circle: bool,
    is_drawing_arrow: bool, // 添加这一行
    // 新增：绘制对象操作状态 - 分开处理
    is_rectangle_dragging: bool,
    is_rectangle_resizing: bool,
    is_circle_dragging: bool,
    is_circle_resizing: bool,
    is_arrow_dragging: bool, // 新增
    is_arrow_resizing: bool, // 新增
    rectangle_resize_mode: String,
    circle_resize_mode: String,
    arrow_resize_mode: String, // 新增："start" 或 "end"
    rectangle_drag_offset_x: f32,
    rectangle_drag_offset_y: f32,
    circle_drag_offset_x: f32,
    circle_drag_offset_y: f32,
    arrow_drag_offset_x: f32, // 新增
    arrow_drag_offset_y: f32, // 新增
    // 画笔相关 - 大幅简化
    current_pen_points: Vec<RustPenPoint>,
    current_pen_commands_cache: String,
    current_pen_bounds_cache: (f32, f32, f32, f32),
    current_pen_cache_valid: bool,
    is_drawing_pen: bool,
    pen_color: String,
    pen_width: f32,
    pen_path_count: u32, // 简单计数
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            selection: SelectionState::default(),
            rectangle_objects: Vec::new(),
            circle_objects: Vec::new(),
            arrow_objects: Vec::new(), // 添加这一行
            selected_object_index: -1,
            drawing_mode: String::new(),
            is_drawing: false,
            selected_icon: String::new(),
            handle_size: 8.0,
            is_drawing_mode: false,
            current_rectangle_object: None,
            current_circle_object: None,
            current_arrow_object: None, // 添加这一行
            is_drawing_rectangle: false,
            is_drawing_circle: false,
            is_drawing_arrow: false, // 添加这一行
            is_rectangle_dragging: false,
            is_rectangle_resizing: false,
            is_circle_dragging: false,
            is_circle_resizing: false,
            is_arrow_dragging: false, // 新增
            is_arrow_resizing: false, // 新增
            rectangle_resize_mode: String::new(),
            circle_resize_mode: String::new(),
            arrow_resize_mode: String::new(), // 新增
            rectangle_drag_offset_x: 0.0,
            rectangle_drag_offset_y: 0.0,
            circle_drag_offset_x: 0.0,
            circle_drag_offset_y: 0.0,
            arrow_drag_offset_x: 0.0, // 新增
            arrow_drag_offset_y: 0.0, // 新增
            // 画笔相关字段
            current_pen_points: Vec::new(),
            current_pen_commands_cache: String::new(),
            current_pen_bounds_cache: (0.0, 0.0, 0.0, 0.0),
            current_pen_cache_valid: false,
            is_drawing_pen: false,
            pen_color: "#ff0044".to_string(),
            pen_width: 3.0,
            pen_path_count: 0,
        }
    }
}

impl AppState {
    fn update_current_pen_path(&mut self) -> (f32, f32, f32, f32, String) {
        if self.current_pen_points.is_empty() {
            return (0.0, 0.0, 0.0, 0.0, String::new());
        }

        if self.current_pen_cache_valid {
            return (
                self.current_pen_bounds_cache.0,
                self.current_pen_bounds_cache.1,
                self.current_pen_bounds_cache.2,
                self.current_pen_bounds_cache.3,
                self.current_pen_commands_cache.clone(),
            );
        }

        let first_point = &self.current_pen_points[0];
        let mut min_x = first_point.x;
        let mut max_x = first_point.x;
        let mut min_y = first_point.y;
        let mut max_y = first_point.y;

        for point in self.current_pen_points.iter().skip(1) {
            min_x = min_x.min(point.x);
            max_x = max_x.max(point.x);
            min_y = min_y.min(point.y);
            max_y = max_y.max(point.y);
        }

        let mut commands = String::new();
        commands.push_str(&format!(
            "M {} {}",
            first_point.x - min_x + 10.0,
            first_point.y - min_y + 10.0
        ));
        for point in self.current_pen_points.iter().skip(1) {
            commands.push_str(&format!(
                " L {} {}",
                point.x - min_x + 10.0,
                point.y - min_y + 10.0
            ));
        }

        self.current_pen_bounds_cache = (min_x, max_x, min_y, max_y);
        self.current_pen_commands_cache = commands.clone();
        self.current_pen_cache_valid = true;

        (min_x, max_x, min_y, max_y, commands)
    }
    fn handle_pen_drawing(&mut self, x: f32, y: f32) {
        if let Some(last_point) = self.current_pen_points.last() {
            let distance = ((x - last_point.x).powi(2) + (y - last_point.y).powi(2)).sqrt();
            if distance < 2.0 {
                return;
            }
        }

        self.current_pen_points.push(RustPenPoint { x, y });
        self.current_pen_cache_valid = false;
    }

    fn start_pen_drawing(&mut self, x: f32, y: f32) {
        self.current_pen_points.clear();
        self.current_pen_points.push(RustPenPoint { x, y });
        self.is_drawing_pen = true;
        self.current_pen_cache_valid = false;
    }

    // 创建独立的画笔路径组件
    fn finish_pen_drawing(&mut self) -> Option<RustPenPath> {
        if !self.current_pen_points.is_empty() && self.current_pen_points.len() > 1 {
            let path = RustPenPath::new(
                self.current_pen_points.clone(),
                self.pen_color.clone(),
                self.pen_width,
            );

            self.pen_path_count += 1;

            // 清理当前绘制状态
            self.current_pen_points.clear();
            self.current_pen_cache_valid = false;
            self.is_drawing_pen = false;

            return Some(path);
        }

        self.current_pen_points.clear();
        self.current_pen_cache_valid = false;
        self.is_drawing_pen = false;
        None
    }

    fn handle_undo_pen(&mut self) {
        if self.is_drawing_pen {
            // 取消当前绘制
            self.current_pen_points.clear();
            self.is_drawing_pen = false;
            self.current_pen_cache_valid = false;
        } else if self.pen_path_count > 0 {
            // 标记需要移除最后一个路径组件
            self.pen_path_count -= 1;
            println!("撤销了路径，剩余: {}", self.pen_path_count);
        }
    }
    fn is_point_in_arrow_object(&self, obj: &RustArrowObject, x: f32, y: f32) -> bool {
        // 计算点到线段的距离
        let line_length =
            ((obj.end_x - obj.start_x).powi(2) + (obj.end_y - obj.start_y).powi(2)).sqrt();
        if line_length == 0.0 {
            return false;
        }

        // 线段向量
        let line_vec_x = (obj.end_x - obj.start_x) / line_length;
        let line_vec_y = (obj.end_y - obj.start_y) / line_length;

        // 点到起点的向量
        let point_vec_x = x - obj.start_x;
        let point_vec_y = y - obj.start_y;

        // 投影长度
        let projection = point_vec_x * line_vec_x + point_vec_y * line_vec_y;

        // 如果投影在线段范围内
        if projection >= 0.0 && projection <= line_length {
            // 计算垂直距离
            let perpendicular_x = point_vec_x - projection * line_vec_x;
            let perpendicular_y = point_vec_y - projection * line_vec_y;
            let distance = (perpendicular_x.powi(2) + perpendicular_y.powi(2)).sqrt();

            distance <= 5.0 // 5像素的容错范围
        } else {
            false
        }
    }

    // 新增：获取箭头对象的控制柄
    fn get_arrow_handle_at_point(&self, obj: &RustArrowObject, x: f32, y: f32) -> String {
        if !obj.selected {
            return String::new();
        }

        let handle_tolerance = self.handle_size + 2.0;
        let half_tolerance = handle_tolerance / 2.0;

        // 检查起点控制柄
        if (x - obj.start_x).abs() <= half_tolerance && (y - obj.start_y).abs() <= half_tolerance {
            return "start".to_string();
        }

        // 检查终点控制柄
        if (x - obj.end_x).abs() <= half_tolerance && (y - obj.end_y).abs() <= half_tolerance {
            return "end".to_string();
        }

        String::new()
    }

    // 新增：处理箭头绘制
    fn handle_arrow_drawing(&mut self, x: f32, y: f32) {
        if let Some(ref mut arrow_obj) = self.current_arrow_object {
            let clamped_x = if self.selection.is_selecting {
                let min_x = self.selection.start_x.min(self.selection.current_x);
                let max_x = self.selection.start_x.max(self.selection.current_x);
                x.max(min_x).min(max_x)
            } else {
                x
            };

            let clamped_y = if self.selection.is_selecting {
                let min_y = self.selection.start_y.min(self.selection.current_y);
                let max_y = self.selection.start_y.max(self.selection.current_y);
                y.max(min_y).min(max_y)
            } else {
                y
            };

            arrow_obj.end_x = clamped_x;
            arrow_obj.end_y = clamped_y;
        }
    }

    // 新增：处理箭头拖拽
    fn handle_arrow_drag(&mut self, x: f32, y: f32, screen_width: f32, screen_height: f32) {
        if let Some(ref mut arrow_obj) = self.current_arrow_object {
            let new_start_x = x - self.arrow_drag_offset_x;
            let new_start_y = y - self.arrow_drag_offset_y;

            let arrow_width = arrow_obj.end_x - arrow_obj.start_x;
            let arrow_height = arrow_obj.end_y - arrow_obj.start_y;

            if self.selection.is_selecting {
                let min_x = self.selection.start_x.min(self.selection.current_x);
                let max_x = self.selection.start_x.max(self.selection.current_x);
                let min_y = self.selection.start_y.min(self.selection.current_y);
                let max_y = self.selection.start_y.max(self.selection.current_y);

                arrow_obj.start_x = new_start_x.max(min_x).min(max_x);
                arrow_obj.start_y = new_start_y.max(min_y).min(max_y);
            } else {
                arrow_obj.start_x = new_start_x.max(0.0).min(screen_width);
                arrow_obj.start_y = new_start_y.max(0.0).min(screen_height);
            }

            arrow_obj.end_x = arrow_obj.start_x + arrow_width;
            arrow_obj.end_y = arrow_obj.start_y + arrow_height;
        }
    }

    // 新增：处理箭头调整大小
    fn handle_arrow_resize(&mut self, x: f32, y: f32, screen_width: f32, screen_height: f32) {
        if let Some(ref mut arrow_obj) = self.current_arrow_object {
            let clamped_x = if self.selection.is_selecting {
                let min_x = self.selection.start_x.min(self.selection.current_x);
                let max_x = self.selection.start_x.max(self.selection.current_x);
                x.max(min_x).min(max_x)
            } else {
                x.max(0.0).min(screen_width)
            };

            let clamped_y = if self.selection.is_selecting {
                let min_y = self.selection.start_y.min(self.selection.current_y);
                let max_y = self.selection.start_y.max(self.selection.current_y);
                y.max(min_y).min(max_y)
            } else {
                y.max(0.0).min(screen_height)
            };

            match self.arrow_resize_mode.as_str() {
                "start" => {
                    arrow_obj.start_x = clamped_x;
                    arrow_obj.start_y = clamped_y;
                }
                "end" => {
                    arrow_obj.end_x = clamped_x;
                    arrow_obj.end_y = clamped_y;
                }
                _ => {}
            }
        }
    }
    fn is_point_in_rectangle_object(&self, obj: &RustRectangleObject, x: f32, y: f32) -> bool {
        let min_x = obj.x.min(obj.x + obj.width);
        let max_x = obj.x.max(obj.x + obj.width);
        let min_y = obj.y.min(obj.y + obj.height);
        let max_y = obj.y.max(obj.y + obj.height);
        x >= min_x && x <= max_x && y >= min_y && y <= max_y
    }
    fn is_point_in_circle_object(&self, obj: &RustCircleObject, x: f32, y: f32) -> bool {
        // 圆形碰撞检测
        let center_x = obj.x + obj.width / 2.0;
        let center_y = obj.y + obj.height / 2.0;
        let radius_x = obj.width / 2.0;
        let radius_y = obj.height / 2.0;

        // 椭圆形碰撞检测公式
        let dx = (x - center_x) / radius_x;
        let dy = (y - center_y) / radius_y;
        dx * dx + dy * dy <= 1.0
    }
    fn get_rectangle_handle_at_point(&self, obj: &RustRectangleObject, x: f32, y: f32) -> String {
        if !obj.selected {
            return String::new();
        }

        let min_x = obj.x.min(obj.x + obj.width);
        let max_x = obj.x.max(obj.x + obj.width);
        let min_y = obj.y.min(obj.y + obj.height);
        let max_y = obj.y.max(obj.y + obj.height);
        let center_x = (min_x + max_x) / 2.0;
        let center_y = (min_y + max_y) / 2.0;

        // 增加手柄检测范围，使其更容易选中
        let handle_tolerance = self.handle_size + 2.0; // 增加2像素的容错
        let half_tolerance = handle_tolerance / 2.0;

        // 检查各个控制柄，增加检测范围
        if (x - min_x).abs() <= half_tolerance && (y - min_y).abs() <= half_tolerance {
            return "nw".to_string();
        }
        if (x - center_x).abs() <= half_tolerance && (y - min_y).abs() <= half_tolerance {
            return "n".to_string();
        }
        if (x - max_x).abs() <= half_tolerance && (y - min_y).abs() <= half_tolerance {
            return "ne".to_string();
        }
        if (x - max_x).abs() <= half_tolerance && (y - center_y).abs() <= half_tolerance {
            return "e".to_string();
        }
        if (x - max_x).abs() <= half_tolerance && (y - max_y).abs() <= half_tolerance {
            return "se".to_string();
        }
        if (x - center_x).abs() <= half_tolerance && (y - max_y).abs() <= half_tolerance {
            return "s".to_string();
        }
        if (x - min_x).abs() <= half_tolerance && (y - max_y).abs() <= half_tolerance {
            return "sw".to_string();
        }
        if (x - min_x).abs() <= half_tolerance && (y - center_y).abs() <= half_tolerance {
            return "w".to_string();
        }

        String::new()
    }

    // 修改：获取圆形对象的控制柄
    fn get_circle_handle_at_point(&self, obj: &RustCircleObject, x: f32, y: f32) -> String {
        if !obj.selected {
            return String::new();
        }

        let min_x = obj.x.min(obj.x + obj.width);
        let max_x = obj.x.max(obj.x + obj.width);
        let min_y = obj.y.min(obj.y + obj.height);
        let max_y = obj.y.max(obj.y + obj.height);
        let center_x = (min_x + max_x) / 2.0;
        let center_y = (min_y + max_y) / 2.0;

        // 增加手柄检测范围，使其更容易选中
        let handle_tolerance = self.handle_size + 2.0; // 增加2像素的容错
        let half_tolerance = handle_tolerance / 2.0;

        // 检查各个控制柄（圆形也使用矩形包围盒的控制柄）
        if (x - min_x).abs() <= half_tolerance && (y - min_y).abs() <= half_tolerance {
            return "nw".to_string();
        }
        if (x - center_x).abs() <= half_tolerance && (y - min_y).abs() <= half_tolerance {
            return "n".to_string();
        }
        if (x - max_x).abs() <= half_tolerance && (y - min_y).abs() <= half_tolerance {
            return "ne".to_string();
        }
        if (x - max_x).abs() <= half_tolerance && (y - center_y).abs() <= half_tolerance {
            return "e".to_string();
        }
        if (x - max_x).abs() <= half_tolerance && (y - max_y).abs() <= half_tolerance {
            return "se".to_string();
        }
        if (x - center_x).abs() <= half_tolerance && (y - max_y).abs() <= half_tolerance {
            return "s".to_string();
        }
        if (x - min_x).abs() <= half_tolerance && (y - max_y).abs() <= half_tolerance {
            return "sw".to_string();
        }
        if (x - min_x).abs() <= half_tolerance && (y - center_y).abs() <= half_tolerance {
            return "w".to_string();
        }

        String::new()
    }
    fn is_point_in_allowed_area(&self, x: f32, y: f32) -> bool {
        // 如果没有选区，则整个屏幕都可以画框
        if !self.selection.is_selecting {
            return true;
        }

        // 如果有选区，只能在选区内画框
        let min_x = self.selection.start_x.min(self.selection.current_x);
        let max_x = self.selection.start_x.max(self.selection.current_x);
        let min_y = self.selection.start_y.min(self.selection.current_y);
        let max_y = self.selection.start_y.max(self.selection.current_y);

        x >= min_x && x <= max_x && y >= min_y && y <= max_y
    }
    // 新增：辅助方法来保存其他类型的对象
    fn save_all_current_objects(&mut self) {
        if let Some(current_obj) = self.current_rectangle_object.take() {
            let mut obj_to_save = current_obj;
            obj_to_save.selected = false;
            self.rectangle_objects.push(obj_to_save);
        }
        if let Some(current_obj) = self.current_circle_object.take() {
            let mut obj_to_save = current_obj;
            obj_to_save.selected = false;
            self.circle_objects.push(obj_to_save);
        }
        if let Some(current_obj) = self.current_arrow_object.take() {
            let mut obj_to_save = current_obj;
            obj_to_save.selected = false;
            self.arrow_objects.push(obj_to_save);
        }
    }

    fn save_other_objects_except_rectangle(&mut self) {
        if let Some(current_obj) = self.current_circle_object.take() {
            let mut obj_to_save = current_obj;
            obj_to_save.selected = false;
            self.circle_objects.push(obj_to_save);
        }
        if let Some(current_obj) = self.current_arrow_object.take() {
            let mut obj_to_save = current_obj;
            obj_to_save.selected = false;
            self.arrow_objects.push(obj_to_save);
        }
    }

    fn save_other_objects_except_circle(&mut self) {
        if let Some(current_obj) = self.current_rectangle_object.take() {
            let mut obj_to_save = current_obj;
            obj_to_save.selected = false;
            self.rectangle_objects.push(obj_to_save);
        }
        if let Some(current_obj) = self.current_arrow_object.take() {
            let mut obj_to_save = current_obj;
            obj_to_save.selected = false;
            self.arrow_objects.push(obj_to_save);
        }
    }

    fn save_other_objects_except_arrow(&mut self) {
        if let Some(current_obj) = self.current_rectangle_object.take() {
            let mut obj_to_save = current_obj;
            obj_to_save.selected = false;
            self.rectangle_objects.push(obj_to_save);
        }
        if let Some(current_obj) = self.current_circle_object.take() {
            let mut obj_to_save = current_obj;
            obj_to_save.selected = false;
            self.circle_objects.push(obj_to_save);
        }
    }
    fn get_mouse_cursor_string(&self, x: f32, y: f32) -> String {
        if self.is_drawing_mode {
            if !self.is_point_in_allowed_area(x, y) {
                return "not-allowed".to_string();
            }

            match self.drawing_mode.as_str() {
                "rectangle" => {
                    if let Some(ref rect_obj) = self.current_rectangle_object {
                        if rect_obj.selected {
                            let handle = self.get_rectangle_handle_at_point(rect_obj, x, y);
                            if !handle.is_empty() {
                                return match handle.as_str() {
                                    "nw" | "se" => "nw-resize".to_string(),
                                    "n" | "s" => "ns-resize".to_string(),
                                    "ne" | "sw" => "ne-resize".to_string(),
                                    "e" | "w" => "ew-resize".to_string(),
                                    _ => "default".to_string(),
                                };
                            }
                            if self.is_point_in_rectangle_object(rect_obj, x, y) {
                                return "move".to_string();
                            }
                        } else {
                            if self.is_point_in_rectangle_object(rect_obj, x, y) {
                                return "pointer".to_string();
                            }
                        }
                    }

                    for rect_obj in &self.rectangle_objects {
                        if self.is_point_in_rectangle_object(rect_obj, x, y) {
                            return "pointer".to_string();
                        }
                    }
                }
                "circle" => {
                    if let Some(ref circle_obj) = self.current_circle_object {
                        if circle_obj.selected {
                            let handle = self.get_circle_handle_at_point(circle_obj, x, y);
                            if !handle.is_empty() {
                                return match handle.as_str() {
                                    "nw" | "se" => "nw-resize".to_string(),
                                    "n" | "s" => "ns-resize".to_string(),
                                    "ne" | "sw" => "ne-resize".to_string(),
                                    "e" | "w" => "ew-resize".to_string(),
                                    _ => "default".to_string(),
                                };
                            }
                            if self.is_point_in_circle_object(circle_obj, x, y) {
                                return "move".to_string();
                            }
                        } else {
                            if self.is_point_in_circle_object(circle_obj, x, y) {
                                return "pointer".to_string();
                            }
                        }
                    }

                    for circle_obj in &self.circle_objects {
                        if self.is_point_in_circle_object(circle_obj, x, y) {
                            return "pointer".to_string();
                        }
                    }
                }
                "arrow" => {
                    if let Some(ref arrow_obj) = self.current_arrow_object {
                        if arrow_obj.selected {
                            let handle = self.get_arrow_handle_at_point(arrow_obj, x, y);
                            if !handle.is_empty() {
                                return "pointer".to_string(); // 箭头的手柄都用 pointer
                            }
                            if self.is_point_in_arrow_object(arrow_obj, x, y) {
                                return "move".to_string();
                            }
                        } else {
                            if self.is_point_in_arrow_object(arrow_obj, x, y) {
                                return "pointer".to_string();
                            }
                        }
                    }

                    for arrow_obj in &self.arrow_objects {
                        if self.is_point_in_arrow_object(arrow_obj, x, y) {
                            return "pointer".to_string();
                        }
                    }
                }
                "pen" => {
                    return "crosshair".to_string(); // 画笔模式使用十字光标
                }
                _ => {}
            }

            return "crosshair".to_string();
        }

        // 原有的选区逻辑
        if !self.selection.current_handle.is_empty() {
            match self.selection.current_handle.as_str() {
                "nw" | "se" => "nw-resize".to_string(),
                "n" | "s" => "ns-resize".to_string(),
                "ne" | "sw" => "ne-resize".to_string(),
                "e" | "w" => "ew-resize".to_string(),
                _ => "default".to_string(),
            }
        } else if self.selection.is_selecting {
            if self.is_point_in_selection(x, y) {
                "crosshair".to_string()
            } else {
                "not-allowed".to_string()
            }
        } else {
            "crosshair".to_string()
        }
    }

    // 新增：获取绘制对象的控制柄

    // 获取指定点处的控制柄
    fn get_handle_at_point(&self, x: f32, y: f32) -> String {
        if !self.selection.is_selecting {
            return String::new();
        }

        let min_x = self.selection.start_x.min(self.selection.current_x);
        let max_x = self.selection.start_x.max(self.selection.current_x);
        let min_y = self.selection.start_y.min(self.selection.current_y);
        let max_y = self.selection.start_y.max(self.selection.current_y);
        let center_x = (min_x + max_x) / 2.0;
        let center_y = (min_y + max_y) / 2.0;
        let half_handle = self.handle_size / 2.0;

        // 检查各个控制柄
        if (x - min_x).abs() <= half_handle && (y - min_y).abs() <= half_handle {
            return "nw".to_string();
        }
        if (x - center_x).abs() <= half_handle && (y - min_y).abs() <= half_handle {
            return "n".to_string();
        }
        if (x - max_x).abs() <= half_handle && (y - min_y).abs() <= half_handle {
            return "ne".to_string();
        }
        if (x - max_x).abs() <= half_handle && (y - center_y).abs() <= half_handle {
            return "e".to_string();
        }
        if (x - max_x).abs() <= half_handle && (y - max_y).abs() <= half_handle {
            return "se".to_string();
        }
        if (x - center_x).abs() <= half_handle && (y - max_y).abs() <= half_handle {
            return "s".to_string();
        }
        if (x - min_x).abs() <= half_handle && (y - max_y).abs() <= half_handle {
            return "sw".to_string();
        }
        if (x - min_x).abs() <= half_handle && (y - center_y).abs() <= half_handle {
            return "w".to_string();
        }

        String::new()
    }

    // 检查点是否在选区内
    fn is_point_in_selection(&self, x: f32, y: f32) -> bool {
        if !self.selection.is_selecting {
            return false;
        }

        let min_x = self.selection.start_x.min(self.selection.current_x);
        let max_x = self.selection.start_x.max(self.selection.current_x);
        let min_y = self.selection.start_y.min(self.selection.current_y);
        let max_y = self.selection.start_y.max(self.selection.current_y);

        x >= min_x && x <= max_x && y >= min_y && y <= max_y
    }

    fn handle_mouse_down(&mut self, x: f32, y: f32) {
        if self.is_drawing_mode {
            if !self.is_point_in_allowed_area(x, y) {
                return;
            }
            // 画笔模式处理
            if self.drawing_mode == "pen" {
                self.start_pen_drawing(x, y);
                return;
            }
            // **最高优先级：检查手柄点击**
            // 检查当前矩形对象的手柄
            if let Some(ref rect_obj) = self.current_rectangle_object {
                if rect_obj.selected {
                    let handle = self.get_rectangle_handle_at_point(rect_obj, x, y);
                    if !handle.is_empty() {
                        self.rectangle_resize_mode = handle;
                        self.is_rectangle_resizing = true;
                        return;
                    }
                }
            }

            // 检查当前圆形对象的手柄
            if let Some(ref circle_obj) = self.current_circle_object {
                if circle_obj.selected {
                    let handle = self.get_circle_handle_at_point(circle_obj, x, y);
                    if !handle.is_empty() {
                        self.circle_resize_mode = handle;
                        self.is_circle_resizing = true;
                        return;
                    }
                }
            }

            // 检查当前箭头对象的手柄
            if let Some(ref arrow_obj) = self.current_arrow_object {
                if arrow_obj.selected {
                    let handle = self.get_arrow_handle_at_point(arrow_obj, x, y);
                    if !handle.is_empty() {
                        self.arrow_resize_mode = handle;
                        self.is_arrow_resizing = true;
                        return;
                    }
                }
            }

            // **第二优先级：检查当前对象的拖拽**
            // 检查当前矩形对象的拖拽
            if let Some(ref rect_obj) = self.current_rectangle_object {
                if self.is_point_in_rectangle_object(rect_obj, x, y) {
                    if self.drawing_mode == "rectangle" {
                        if let Some(ref mut rect_obj) = self.current_rectangle_object {
                            rect_obj.selected = true;
                            self.is_rectangle_dragging = true;
                            self.rectangle_drag_offset_x = x - rect_obj.x;
                            self.rectangle_drag_offset_y = y - rect_obj.y;
                        }
                        return;
                    } else {
                        self.save_other_objects_except_rectangle();
                        self.drawing_mode = "rectangle".to_string();
                        if let Some(ref mut rect_obj) = self.current_rectangle_object {
                            rect_obj.selected = true;
                            self.is_rectangle_dragging = true;
                            self.rectangle_drag_offset_x = x - rect_obj.x;
                            self.rectangle_drag_offset_y = y - rect_obj.y;
                        }
                        return;
                    }
                }
            }

            // 检查当前圆形对象的拖拽
            if let Some(ref circle_obj) = self.current_circle_object {
                if self.is_point_in_circle_object(circle_obj, x, y) {
                    if self.drawing_mode == "circle" {
                        if let Some(ref mut circle_obj) = self.current_circle_object {
                            circle_obj.selected = true;
                            self.is_circle_dragging = true;
                            self.circle_drag_offset_x = x - circle_obj.x;
                            self.circle_drag_offset_y = y - circle_obj.y;
                        }
                        return;
                    } else {
                        self.save_other_objects_except_circle();
                        self.drawing_mode = "circle".to_string();
                        if let Some(ref mut circle_obj) = self.current_circle_object {
                            circle_obj.selected = true;
                            self.is_circle_dragging = true;
                            self.circle_drag_offset_x = x - circle_obj.x;
                            self.circle_drag_offset_y = y - circle_obj.y;
                        }
                        return;
                    }
                }
            }

            // 检查当前箭头对象的拖拽
            if let Some(ref arrow_obj) = self.current_arrow_object {
                if self.is_point_in_arrow_object(arrow_obj, x, y) {
                    if self.drawing_mode == "arrow" {
                        if let Some(ref mut arrow_obj) = self.current_arrow_object {
                            arrow_obj.selected = true;
                            self.is_arrow_dragging = true;
                            self.arrow_drag_offset_x = x - arrow_obj.start_x;
                            self.arrow_drag_offset_y = y - arrow_obj.start_y;
                        }
                        return;
                    } else {
                        self.save_other_objects_except_arrow();
                        self.drawing_mode = "arrow".to_string();
                        if let Some(ref mut arrow_obj) = self.current_arrow_object {
                            arrow_obj.selected = true;
                            self.is_arrow_dragging = true;
                            self.arrow_drag_offset_x = x - arrow_obj.start_x;
                            self.arrow_drag_offset_y = y - arrow_obj.start_y;
                        }
                        return;
                    }
                }
            }

            // **第三优先级：检查保存的对象**
            // 检查保存的矩形对象
            for (index, rect_obj) in self.rectangle_objects.iter().enumerate() {
                if self.is_point_in_rectangle_object(rect_obj, x, y) {
                    self.save_all_current_objects();

                    let mut selected_obj = self.rectangle_objects.remove(index);
                    selected_obj.selected = true;

                    self.drawing_mode = "rectangle".to_string();
                    self.is_rectangle_dragging = true;
                    self.rectangle_drag_offset_x = x - selected_obj.x;
                    self.rectangle_drag_offset_y = y - selected_obj.y;

                    self.current_rectangle_object = Some(selected_obj);
                    return;
                }
            }

            // 检查保存的圆形对象
            for (index, circle_obj) in self.circle_objects.iter().enumerate() {
                if self.is_point_in_circle_object(circle_obj, x, y) {
                    self.save_all_current_objects();

                    let mut selected_obj = self.circle_objects.remove(index);
                    selected_obj.selected = true;

                    self.drawing_mode = "circle".to_string();
                    self.is_circle_dragging = true;
                    self.circle_drag_offset_x = x - selected_obj.x;
                    self.circle_drag_offset_y = y - selected_obj.y;

                    self.current_circle_object = Some(selected_obj);
                    return;
                }
            }

            // 检查保存的箭头对象
            for (index, arrow_obj) in self.arrow_objects.iter().enumerate() {
                if self.is_point_in_arrow_object(arrow_obj, x, y) {
                    self.save_all_current_objects();

                    let mut selected_obj = self.arrow_objects.remove(index);
                    selected_obj.selected = true;

                    self.drawing_mode = "arrow".to_string();
                    self.is_arrow_dragging = true;
                    self.arrow_drag_offset_x = x - selected_obj.start_x;
                    self.arrow_drag_offset_y = y - selected_obj.start_y;

                    self.current_arrow_object = Some(selected_obj);
                    return;
                }
            }

            // **最低优先级：创建新对象**
            match self.drawing_mode.as_str() {
                "rectangle" => {
                    self.save_other_objects_except_rectangle();
                    if let Some(current_obj) = self.current_rectangle_object.take() {
                        let mut obj_to_save = current_obj;
                        obj_to_save.selected = false;
                        self.rectangle_objects.push(obj_to_save);
                    }

                    self.current_rectangle_object = Some(RustRectangleObject {
                        x,
                        y,
                        width: 0.0,
                        height: 0.0,
                        start_x: x,
                        start_y: y,
                        end_x: x,
                        end_y: y,
                        selected: true,
                    });
                    self.is_drawing_rectangle = true;
                }
                "circle" => {
                    self.save_other_objects_except_circle();
                    if let Some(current_obj) = self.current_circle_object.take() {
                        let mut obj_to_save = current_obj;
                        obj_to_save.selected = false;
                        self.circle_objects.push(obj_to_save);
                    }

                    self.current_circle_object = Some(RustCircleObject {
                        x,
                        y,
                        width: 0.0,
                        height: 0.0,
                        start_x: x,
                        start_y: y,
                        end_x: x,
                        end_y: y,
                        selected: true,
                    });
                    self.is_drawing_circle = true;
                }
                "arrow" => {
                    self.save_other_objects_except_arrow();
                    if let Some(current_obj) = self.current_arrow_object.take() {
                        let mut obj_to_save = current_obj;
                        obj_to_save.selected = false;
                        self.arrow_objects.push(obj_to_save);
                    }

                    self.current_arrow_object = Some(RustArrowObject {
                        start_x: x,
                        start_y: y,
                        end_x: x,
                        end_y: y,
                        selected: true,
                    });
                    self.is_drawing_arrow = true;
                }
                _ => {}
            }
            return;
        }

        // 原有的选区逻辑保持不变...
        self.selection.current_handle = self.get_handle_at_point(x, y);

        if !self.selection.current_handle.is_empty() {
            self.selection.is_resizing = true;
            self.selection.resize_mode = self.selection.current_handle.clone();
        } else if self.selection.is_selecting && self.is_point_in_selection(x, y) {
            self.selection.is_dragging = true;
            let min_x = self.selection.start_x.min(self.selection.current_x);
            let min_y = self.selection.start_y.min(self.selection.current_y);
            self.selection.drag_offset_x = x - min_x;
            self.selection.drag_offset_y = y - min_y;
        } else if self.selection.is_selecting {
        } else {
            self.selection.start_x = x;
            self.selection.start_y = y;
            self.selection.current_x = x;
            self.selection.current_y = y;
            self.selection.is_selecting = true;
            self.selection.is_drawing = true;
            self.selection.is_dragging = false;
            self.selection.is_resizing = false;
        }
    }

    // 处理鼠标移动事件
    fn handle_mouse_move(&mut self, x: f32, y: f32, screen_width: f32, screen_height: f32) {
        if self.is_drawing_mode {
            match self.drawing_mode.as_str() {
                "rectangle" => {
                    if self.is_drawing_rectangle {
                        self.handle_rectangle_drawing(x, y);
                        return;
                    } else if self.is_rectangle_resizing {
                        self.handle_rectangle_resize(x, y, screen_width, screen_height);
                        return;
                    } else if self.is_rectangle_dragging {
                        self.handle_rectangle_drag(x, y, screen_width, screen_height);
                        return;
                    }
                }
                "circle" => {
                    if self.is_drawing_circle {
                        self.handle_circle_drawing(x, y);
                        return;
                    } else if self.is_circle_resizing {
                        self.handle_circle_resize(x, y, screen_width, screen_height);
                        return;
                    } else if self.is_circle_dragging {
                        self.handle_circle_drag(x, y, screen_width, screen_height);
                        return;
                    }
                }
                "arrow" => {
                    if self.is_drawing_arrow {
                        self.handle_arrow_drawing(x, y);
                        return;
                    } else if self.is_arrow_resizing {
                        self.handle_arrow_resize(x, y, screen_width, screen_height);
                        return;
                    } else if self.is_arrow_dragging {
                        self.handle_arrow_drag(x, y, screen_width, screen_height);
                        return;
                    }
                }
                "pen" => {
                    if self.is_drawing_pen {
                        self.handle_pen_drawing(x, y);
                        return;
                    }
                }
                _ => {}
            }
            return;
        }

        // 原有的选区移动逻辑
        if self.selection.is_resizing {
            self.handle_resize(x, y, screen_width, screen_height);
        } else if self.selection.is_dragging {
            self.handle_drag(x, y, screen_width, screen_height);
        } else if self.selection.is_drawing {
            self.selection.current_x = x;
            self.selection.current_y = y;
        }

        if !self.selection.is_resizing && !self.selection.is_dragging && !self.selection.is_drawing
        {
            self.selection.current_handle = self.get_handle_at_point(x, y);
        }
    }

    fn handle_rectangle_drawing(&mut self, x: f32, y: f32) {
        if let Some(ref mut rect_obj) = self.current_rectangle_object {
            let clamped_x = if self.selection.is_selecting {
                let min_x = self.selection.start_x.min(self.selection.current_x);
                let max_x = self.selection.start_x.max(self.selection.current_x);
                x.max(min_x).min(max_x)
            } else {
                x
            };

            let clamped_y = if self.selection.is_selecting {
                let min_y = self.selection.start_y.min(self.selection.current_y);
                let max_y = self.selection.start_y.max(self.selection.current_y);
                y.max(min_y).min(max_y)
            } else {
                y
            };

            let raw_width = clamped_x - rect_obj.start_x;
            let raw_height = clamped_y - rect_obj.start_y;

            // 根据拖拽方向调整矩形的位置和大小
            if raw_width >= 0.0 && raw_height >= 0.0 {
                rect_obj.x = rect_obj.start_x;
                rect_obj.y = rect_obj.start_y;
                rect_obj.width = raw_width;
                rect_obj.height = raw_height;
            } else if raw_width < 0.0 && raw_height >= 0.0 {
                rect_obj.x = clamped_x;
                rect_obj.y = rect_obj.start_y;
                rect_obj.width = -raw_width;
                rect_obj.height = raw_height;
            } else if raw_width >= 0.0 && raw_height < 0.0 {
                rect_obj.x = rect_obj.start_x;
                rect_obj.y = clamped_y;
                rect_obj.width = raw_width;
                rect_obj.height = -raw_height;
            } else {
                rect_obj.x = clamped_x;
                rect_obj.y = clamped_y;
                rect_obj.width = -raw_width;
                rect_obj.height = -raw_height;
            }

            rect_obj.end_x = clamped_x;
            rect_obj.end_y = clamped_y;
        }
    }

    // 新增：处理圆形绘制
    fn handle_circle_drawing(&mut self, x: f32, y: f32) {
        if let Some(ref mut circle_obj) = self.current_circle_object {
            let clamped_x = if self.selection.is_selecting {
                let min_x = self.selection.start_x.min(self.selection.current_x);
                let max_x = self.selection.start_x.max(self.selection.current_x);
                x.max(min_x).min(max_x)
            } else {
                x
            };

            let clamped_y = if self.selection.is_selecting {
                let min_y = self.selection.start_y.min(self.selection.current_y);
                let max_y = self.selection.start_y.max(self.selection.current_y);
                y.max(min_y).min(max_y)
            } else {
                y
            };

            let raw_width = clamped_x - circle_obj.start_x;
            let raw_height = clamped_y - circle_obj.start_y;

            // 根据拖拽方向调整圆形的位置和大小
            if raw_width >= 0.0 && raw_height >= 0.0 {
                circle_obj.x = circle_obj.start_x;
                circle_obj.y = circle_obj.start_y;
                circle_obj.width = raw_width;
                circle_obj.height = raw_height;
            } else if raw_width < 0.0 && raw_height >= 0.0 {
                circle_obj.x = clamped_x;
                circle_obj.y = circle_obj.start_y;
                circle_obj.width = -raw_width;
                circle_obj.height = raw_height;
            } else if raw_width >= 0.0 && raw_height < 0.0 {
                circle_obj.x = circle_obj.start_x;
                circle_obj.y = clamped_y;
                circle_obj.width = raw_width;
                circle_obj.height = -raw_height;
            } else {
                circle_obj.x = clamped_x;
                circle_obj.y = clamped_y;
                circle_obj.width = -raw_width;
                circle_obj.height = -raw_height;
            }

            circle_obj.end_x = clamped_x;
            circle_obj.end_y = clamped_y;
        }
    }

    // 新增：处理矩形拖拽
    fn handle_rectangle_drag(&mut self, x: f32, y: f32, screen_width: f32, screen_height: f32) {
        if let Some(ref mut rect_obj) = self.current_rectangle_object {
            let new_x = x - self.rectangle_drag_offset_x;
            let new_y = y - self.rectangle_drag_offset_y;

            if self.selection.is_selecting {
                let min_x = self.selection.start_x.min(self.selection.current_x);
                let max_x = self.selection.start_x.max(self.selection.current_x);
                let min_y = self.selection.start_y.min(self.selection.current_y);
                let max_y = self.selection.start_y.max(self.selection.current_y);

                rect_obj.x = new_x.max(min_x).min(max_x - rect_obj.width);
                rect_obj.y = new_y.max(min_y).min(max_y - rect_obj.height);
            } else {
                rect_obj.x = new_x.max(0.0).min(screen_width - rect_obj.width);
                rect_obj.y = new_y.max(0.0).min(screen_height - rect_obj.height);
            }
        }
    }

    // 新增：处理圆形拖拽
    fn handle_circle_drag(&mut self, x: f32, y: f32, screen_width: f32, screen_height: f32) {
        if let Some(ref mut circle_obj) = self.current_circle_object {
            let new_x = x - self.circle_drag_offset_x;
            let new_y = y - self.circle_drag_offset_y;

            if self.selection.is_selecting {
                let min_x = self.selection.start_x.min(self.selection.current_x);
                let max_x = self.selection.start_x.max(self.selection.current_x);
                let min_y = self.selection.start_y.min(self.selection.current_y);
                let max_y = self.selection.start_y.max(self.selection.current_y);

                circle_obj.x = new_x.max(min_x).min(max_x - circle_obj.width);
                circle_obj.y = new_y.max(min_y).min(max_y - circle_obj.height);
            } else {
                circle_obj.x = new_x.max(0.0).min(screen_width - circle_obj.width);
                circle_obj.y = new_y.max(0.0).min(screen_height - circle_obj.height);
            }
        }
    }

    // 新增：处理矩形调整大小
    fn handle_rectangle_resize(&mut self, x: f32, y: f32, screen_width: f32, screen_height: f32) {
        if let Some(ref mut rect_obj) = self.current_rectangle_object {
            match self.rectangle_resize_mode.as_str() {
                "nw" => {
                    let new_width = rect_obj.x + rect_obj.width - x;
                    let new_height = rect_obj.y + rect_obj.height - y;
                    if new_width >= 10.0 && new_height >= 10.0 {
                        rect_obj.x = x;
                        rect_obj.y = y;
                        rect_obj.width = new_width;
                        rect_obj.height = new_height;
                    }
                }
                "n" => {
                    let new_height = rect_obj.y + rect_obj.height - y;
                    if new_height >= 10.0 {
                        rect_obj.y = y;
                        rect_obj.height = new_height;
                    }
                }
                "ne" => {
                    let new_width = x - rect_obj.x;
                    let new_height = rect_obj.y + rect_obj.height - y;
                    if new_width >= 10.0 && new_height >= 10.0 {
                        rect_obj.width = new_width;
                        rect_obj.y = y;
                        rect_obj.height = new_height;
                    }
                }
                "e" => {
                    let new_width = x - rect_obj.x;
                    if new_width >= 10.0 {
                        rect_obj.width = new_width;
                    }
                }
                "se" => {
                    let new_width = x - rect_obj.x;
                    let new_height = y - rect_obj.y;
                    if new_width >= 10.0 && new_height >= 10.0 {
                        rect_obj.width = new_width;
                        rect_obj.height = new_height;
                    }
                }
                "s" => {
                    let new_height = y - rect_obj.y;
                    if new_height >= 10.0 {
                        rect_obj.height = new_height;
                    }
                }
                "sw" => {
                    let new_width = rect_obj.x + rect_obj.width - x;
                    let new_height = y - rect_obj.y;
                    if new_width >= 10.0 && new_height >= 10.0 {
                        rect_obj.x = x;
                        rect_obj.width = new_width;
                        rect_obj.height = new_height;
                    }
                }
                "w" => {
                    let new_width = rect_obj.x + rect_obj.width - x;
                    if new_width >= 10.0 {
                        rect_obj.x = x;
                        rect_obj.width = new_width;
                    }
                }
                _ => {}
            }
        }
    }

    // 新增：处理圆形调整大小
    fn handle_circle_resize(&mut self, x: f32, y: f32, screen_width: f32, screen_height: f32) {
        if let Some(ref mut circle_obj) = self.current_circle_object {
            match self.circle_resize_mode.as_str() {
                "nw" => {
                    let new_width = circle_obj.x + circle_obj.width - x;
                    let new_height = circle_obj.y + circle_obj.height - y;
                    if new_width >= 10.0 && new_height >= 10.0 {
                        circle_obj.x = x;
                        circle_obj.y = y;
                        circle_obj.width = new_width;
                        circle_obj.height = new_height;
                    }
                }
                "n" => {
                    let new_height = circle_obj.y + circle_obj.height - y;
                    if new_height >= 10.0 {
                        circle_obj.y = y;
                        circle_obj.height = new_height;
                    }
                }
                "ne" => {
                    let new_width = x - circle_obj.x;
                    let new_height = circle_obj.y + circle_obj.height - y;
                    if new_width >= 10.0 && new_height >= 10.0 {
                        circle_obj.width = new_width;
                        circle_obj.y = y;
                        circle_obj.height = new_height;
                    }
                }
                "e" => {
                    let new_width = x - circle_obj.x;
                    if new_width >= 10.0 {
                        circle_obj.width = new_width;
                    }
                }
                "se" => {
                    let new_width = x - circle_obj.x;
                    let new_height = y - circle_obj.y;
                    if new_width >= 10.0 && new_height >= 10.0 {
                        circle_obj.width = new_width;
                        circle_obj.height = new_height;
                    }
                }
                "s" => {
                    let new_height = y - circle_obj.y;
                    if new_height >= 10.0 {
                        circle_obj.height = new_height;
                    }
                }
                "sw" => {
                    let new_width = circle_obj.x + circle_obj.width - x;
                    let new_height = y - circle_obj.y;
                    if new_width >= 10.0 && new_height >= 10.0 {
                        circle_obj.x = x;
                        circle_obj.width = new_width;
                        circle_obj.height = new_height;
                    }
                }
                "w" => {
                    let new_width = circle_obj.x + circle_obj.width - x;
                    if new_width >= 10.0 {
                        circle_obj.x = x;
                        circle_obj.width = new_width;
                    }
                }
                _ => {}
            }
        }
    }
    // 新增：处理绘制对象的调整大小

    // 处理调整大小
    fn handle_resize(&mut self, x: f32, y: f32, screen_width: f32, screen_height: f32) {
        match self.selection.resize_mode.as_str() {
            "nw" => {
                self.selection.start_x = (0.0_f32).max((x).min(self.selection.current_x - 10.0));
                self.selection.start_y = (0.0_f32).max((y).min(self.selection.current_y - 10.0));
            }
            "n" => {
                self.selection.start_y = (0.0_f32).max((y).min(self.selection.current_y - 10.0));
            }
            "ne" => {
                self.selection.current_x = screen_width.min((x).max(self.selection.start_x + 10.0));
                self.selection.start_y = (0.0_f32).max((y).min(self.selection.current_y - 10.0));
            }
            "e" => {
                self.selection.current_x = screen_width.min((x).max(self.selection.start_x + 10.0));
            }
            "se" => {
                self.selection.current_x = screen_width.min((x).max(self.selection.start_x + 10.0));
                self.selection.current_y =
                    screen_height.min((y).max(self.selection.start_y + 10.0));
            }
            "s" => {
                self.selection.current_y =
                    screen_height.min((y).max(self.selection.start_y + 10.0));
            }
            "sw" => {
                self.selection.start_x = (0.0_f32).max((x).min(self.selection.current_x - 10.0));
                self.selection.current_y =
                    screen_height.min((y).max(self.selection.start_y + 10.0));
            }
            "w" => {
                self.selection.start_x = (0.0_f32).max((x).min(self.selection.current_x - 10.0));
            }
            _ => {}
        }
    }

    // 处理拖拽
    fn handle_drag(&mut self, x: f32, y: f32, screen_width: f32, screen_height: f32) {
        let box_width = (self.selection.current_x - self.selection.start_x).abs();
        let box_height = (self.selection.current_y - self.selection.start_y).abs();

        self.selection.start_x =
            (0.0_f32).max((x - self.selection.drag_offset_x).min(screen_width - box_width));
        self.selection.start_y =
            (0.0_f32).max((y - self.selection.drag_offset_y).min(screen_height - box_height));
        self.selection.current_x = self.selection.start_x + box_width;
        self.selection.current_y = self.selection.start_y + box_height;
    }

    // 处理鼠标释放事件
    fn handle_mouse_up(&mut self) {
        if self.is_drawing_mode {
            match self.drawing_mode.as_str() {
                "rectangle" => {
                    if self.is_drawing_rectangle {
                        if let Some(ref mut rect_obj) = self.current_rectangle_object {
                            let width = rect_obj.width.abs();
                            let height = rect_obj.height.abs();

                            if width >= 5.0 && height >= 5.0 {
                                rect_obj.width = width;
                                rect_obj.height = height;
                                rect_obj.selected = true;
                            } else {
                                self.current_rectangle_object = None;
                            }
                        }
                        self.is_drawing_rectangle = false;
                        return;
                    } else if self.is_rectangle_resizing {
                        self.is_rectangle_resizing = false;
                        self.rectangle_resize_mode.clear();
                        return;
                    } else if self.is_rectangle_dragging {
                        self.is_rectangle_dragging = false;
                        if let Some(ref mut rect_obj) = self.current_rectangle_object {
                            rect_obj.selected = true;
                        }
                        return;
                    }
                }
                "circle" => {
                    if self.is_drawing_circle {
                        if let Some(ref mut circle_obj) = self.current_circle_object {
                            let width = circle_obj.width.abs();
                            let height = circle_obj.height.abs();

                            if width >= 5.0 && height >= 5.0 {
                                circle_obj.width = width;
                                circle_obj.height = height;
                                circle_obj.selected = true;
                            } else {
                                self.current_circle_object = None;
                            }
                        }
                        self.is_drawing_circle = false;
                        return;
                    } else if self.is_circle_resizing {
                        self.is_circle_resizing = false;
                        self.circle_resize_mode.clear();
                        return;
                    } else if self.is_circle_dragging {
                        self.is_circle_dragging = false;
                        if let Some(ref mut circle_obj) = self.current_circle_object {
                            circle_obj.selected = true;
                        }
                        return;
                    }
                }
                "arrow" => {
                    if self.is_drawing_arrow {
                        if let Some(ref mut arrow_obj) = self.current_arrow_object {
                            let length = ((arrow_obj.end_x - arrow_obj.start_x).powi(2)
                                + (arrow_obj.end_y - arrow_obj.start_y).powi(2))
                            .sqrt();

                            if length >= 10.0 {
                                arrow_obj.selected = true;
                            } else {
                                self.current_arrow_object = None;
                            }
                        }
                        self.is_drawing_arrow = false;
                        return;
                    } else if self.is_arrow_resizing {
                        self.is_arrow_resizing = false;
                        self.arrow_resize_mode.clear();
                        return;
                    } else if self.is_arrow_dragging {
                        self.is_arrow_dragging = false;
                        if let Some(ref mut arrow_obj) = self.current_arrow_object {
                            arrow_obj.selected = true;
                        }
                        return;
                    }
                }
                "pen" => {
                    if self.is_drawing_pen {
                        self.finish_pen_drawing();
                        return;
                    }
                }
                _ => {}
            }
            return;
        }

        // 原有的选区释放逻辑
        if self.selection.is_resizing {
            self.selection.is_resizing = false;
            self.selection.resize_mode.clear();
        } else if self.selection.is_dragging {
            self.selection.is_dragging = false;
        } else if self.selection.is_drawing {
            self.selection.is_drawing = false;
            let width = (self.selection.current_x - self.selection.start_x).abs();
            let height = (self.selection.current_y - self.selection.start_y).abs();

            if width < 5.0 || height < 5.0 {
                self.selection.is_selecting = false;
            }
        }
    }
    // 处理工具栏按钮点击
    fn handle_toolbar_click(&mut self, icon_name: &str) -> (Option<SelectionArea>, bool) {
        self.selected_icon = icon_name.to_string();

        match icon_name {
            "square" => {
                // 保存当前圆形对象到列表（如果存在）
                if let Some(current_obj) = self.current_circle_object.take() {
                    let mut obj_to_save = current_obj;
                    obj_to_save.selected = false;
                    self.circle_objects.push(obj_to_save);
                }
                // 保存当前箭头对象
                if let Some(current_obj) = self.current_arrow_object.take() {
                    let mut obj_to_save = current_obj;
                    obj_to_save.selected = false;
                    self.arrow_objects.push(obj_to_save);
                }

                self.is_drawing_mode = true;
                self.drawing_mode = "rectangle".to_string();
                self.is_drawing_rectangle = false;
                self.is_drawing_circle = false;
                self.is_drawing_arrow = false;
                (None, false)
            }
            "circle" => {
                // 保存当前矩形对象到列表（如果存在）
                if let Some(current_obj) = self.current_rectangle_object.take() {
                    let mut obj_to_save = current_obj;
                    obj_to_save.selected = false;
                    self.rectangle_objects.push(obj_to_save);
                }
                // 保存当前箭头对象
                if let Some(current_obj) = self.current_arrow_object.take() {
                    let mut obj_to_save = current_obj;
                    obj_to_save.selected = false;
                    self.arrow_objects.push(obj_to_save);
                }

                self.is_drawing_mode = true;
                self.drawing_mode = "circle".to_string();
                self.is_drawing_rectangle = false;
                self.is_drawing_circle = false;
                self.is_drawing_arrow = false;
                (None, false)
            }
            "arrow" => {
                self.save_other_objects_except_arrow();
                self.is_drawing_mode = true;
                self.drawing_mode = "arrow".to_string();
                self.is_drawing_rectangle = false;
                self.is_drawing_circle = false;
                self.is_drawing_arrow = false;
                (None, false)
            }
            "pen" => {
                // 保存所有当前对象
                self.save_all_current_objects();
                self.is_drawing_mode = true;
                self.drawing_mode = "pen".to_string();
                (None, false)
            }
            "clipboard" => {
                if self.selection.is_selecting {
                    let area = SelectionArea {
                        x: self.selection.start_x.min(self.selection.current_x),
                        y: self.selection.start_y.min(self.selection.current_y),
                        width: (self.selection.current_x - self.selection.start_x).abs(),
                        height: (self.selection.current_y - self.selection.start_y).abs(),
                    };
                    return (Some(area), false);
                }
                (None, false)
            }
            "undo" => {
                match self.drawing_mode.as_str() {
                    "pen" => {
                        self.handle_undo_pen();
                    }
                    "rectangle" => {
                        if let Some(_) = self.current_rectangle_object.take() {
                            // 移除当前矩形
                        } else if !self.rectangle_objects.is_empty() {
                            self.rectangle_objects.pop();
                        } else if !self.circle_objects.is_empty() {
                            self.circle_objects.pop();
                        } else if !self.arrow_objects.is_empty() {
                            self.arrow_objects.pop();
                        }
                    }
                    "circle" => {
                        if let Some(_) = self.current_circle_object.take() {
                            // 移除当前圆形
                        } else if !self.circle_objects.is_empty() {
                            self.circle_objects.pop();
                        } else if !self.rectangle_objects.is_empty() {
                            self.rectangle_objects.pop();
                        } else if !self.arrow_objects.is_empty() {
                            self.arrow_objects.pop();
                        }
                    }
                    "arrow" => {
                        if let Some(_) = self.current_arrow_object.take() {
                            // 移除当前箭头
                        } else if !self.arrow_objects.is_empty() {
                            self.arrow_objects.pop();
                        } else if !self.rectangle_objects.is_empty() {
                            self.rectangle_objects.pop();
                        } else if !self.circle_objects.is_empty() {
                            self.circle_objects.pop();
                        }
                    }
                    _ => {
                        // 智能撤销：按照最后创建的顺序撤销
                        if !self.rectangle_objects.is_empty()
                            || !self.circle_objects.is_empty()
                            || !self.arrow_objects.is_empty()
                            || self.pen_path_count > 0
                        {
                            self.save_all_current_objects();

                            // 按照最后创建的顺序撤销
                            if self.pen_path_count > 0 {
                                self.handle_undo_pen();
                            } else if !self.arrow_objects.is_empty() {
                                self.arrow_objects.pop();
                            } else if !self.circle_objects.is_empty() {
                                self.circle_objects.pop();
                            } else if !self.rectangle_objects.is_empty() {
                                self.rectangle_objects.pop();
                            }
                        } else if self.selection.is_selecting {
                            self.selection.is_selecting = false;
                            self.selection.is_drawing = false;
                            self.selection.is_dragging = false;
                            self.selection.is_resizing = false;
                        }
                    }
                }
                (None, false)
            }
            "download" => {
                if self.selection.is_selecting {
                    let area = SelectionArea {
                        x: self.selection.start_x.min(self.selection.current_x),
                        y: self.selection.start_y.min(self.selection.current_y),
                        width: (self.selection.current_x - self.selection.start_x).abs(),
                        height: (self.selection.current_y - self.selection.start_y).abs(),
                    };
                    return (Some(area), false);
                }
                (None, false)
            }
            "close" => (None, true),
            "check" => {
                if self.selection.is_selecting {
                    let area = SelectionArea {
                        x: self.selection.start_x.min(self.selection.current_x),
                        y: self.selection.start_y.min(self.selection.current_y),
                        width: (self.selection.current_x - self.selection.start_x).abs(),
                        height: (self.selection.current_y - self.selection.start_y).abs(),
                    };
                    return (Some(area), false);
                }
                (None, false)
            }
            _ => (None, false),
        }
    }

    // 获取鼠标光标类型
}

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
    if let Ok(screens) = Screen::all() {
        if let Some(screen) = screens.first() {
            if let Ok(image) = screen.capture() {
                let width = image.width();
                let height = image.height();
                let background_data = image.into_raw();

                let app = AppWindow::new()?;
                let app_state = Rc::new(RefCell::new(AppState::default()));

                // 设置背景图像
                let mut pixel_buffer =
                    slint::SharedPixelBuffer::<slint::Rgba8Pixel>::new(width, height);
                let buffer = pixel_buffer.make_mut_bytes();
                buffer.copy_from_slice(&background_data);

                app.window()
                    .set_position(LogicalPosition::new(0 as f32, 0 as f32));
                let background_image = slint::Image::from_rgba8(pixel_buffer);
                app.set_background_screenshot(background_image);
                app.set_show_mask(true);

                // 处理鼠标事件
                let app_weak = app.as_weak();
                let app_state_clone = app_state.clone();
                app.on_mouse_event(move |event_type, x, y| {
                    if let Some(app) = app_weak.upgrade() {
                        let mut state = app_state_clone.borrow_mut();

                        // 用于处理画笔路径创建的标志
                        let mut should_create_pen_path = false;
                        let mut pen_path_data = None;

                        match event_type.as_str() {
                            "down" => state.handle_mouse_down(x, y),
                            "move" => {
                                // 针对画笔模式优化鼠标移动处理
                                if state.drawing_mode == "pen" && state.is_drawing_pen {
                                    // 画笔模式下直接处理，减少延迟
                                    state.handle_pen_drawing(x, y);
                                } else {
                                    state.handle_mouse_move(x, y, width as f32, height as f32);
                                }
                            }
                            "up" => {
                                // 检查是否需要创建画笔路径
                                if state.drawing_mode == "pen" && state.is_drawing_pen {
                                    if let Some(path) = state.finish_pen_drawing() {
                                        should_create_pen_path = true;
                                        pen_path_data = Some((
                                            path.min_x,
                                            path.max_x,
                                            path.min_y,
                                            path.max_y,
                                            path.commands.clone(),
                                            state.pen_color.clone(),
                                            path.width,
                                        ));
                                    }
                                } else {
                                    state.handle_mouse_up();
                                }
                            }
                            _ => {}
                        }

                        // 如果需要创建画笔路径，调用回调
                        if should_create_pen_path {
                            if let Some((min_x, max_x, min_y, max_y, commands, color, width)) =
                                pen_path_data
                            {
                                app.invoke_create_pen_path_component(
                                    min_x,
                                    max_x,
                                    min_y,
                                    max_y,
                                    commands.into(),
                                    slint::Color::from_argb_encoded(
                                        u32::from_str_radix(&color[1..], 16).unwrap_or(0xff0044)
                                            | 0xff000000,
                                    ),
                                    width,
                                );
                            }
                        }

                        // 更新UI状态
                        app.set_start_x(state.selection.start_x);
                        app.set_start_y(state.selection.start_y);
                        app.set_current_x(state.selection.current_x);
                        app.set_current_y(state.selection.current_y);
                        app.set_is_selecting(state.selection.is_selecting);
                        app.set_cursor_type(state.get_mouse_cursor_string(x, y).into());

                        // 更新画框模式状态
                        app.set_is_drawing_mode(state.is_drawing_mode);

                        // 只在画笔模式下更新画笔相关状态
                        if state.drawing_mode == "pen" {
                            app.set_show_current_pen_path(!state.current_pen_points.is_empty());

                            let (min_x, max_x, min_y, max_y, commands) =
                                state.update_current_pen_path();
                            app.set_current_pen_min_x(min_x);
                            app.set_current_pen_max_x(max_x);
                            app.set_current_pen_min_y(min_y);
                            app.set_current_pen_max_y(max_y);
                            app.set_current_pen_path_commands(commands.into());
                        } else {
                            app.set_show_current_pen_path(false);
                        }

                        // 分别更新保存的对象列表
                        let saved_rectangles: Vec<_> = state
                            .rectangle_objects
                            .iter()
                            .map(|obj| slint_generatedAppWindow::RectangleObject {
                                x: obj.x,
                                y: obj.y,
                                width: obj.width,
                                height: obj.height,
                                selected: obj.selected,
                            })
                            .collect();
                        app.set_saved_rectangles(saved_rectangles.as_slice().into());

                        let saved_circles: Vec<_> = state
                            .circle_objects
                            .iter()
                            .map(|obj| slint_generatedAppWindow::CircleObject {
                                x: obj.x,
                                y: obj.y,
                                width: obj.width,
                                height: obj.height,
                                selected: obj.selected,
                            })
                            .collect();
                        app.set_saved_circles(saved_circles.as_slice().into());

                        // 更新保存的箭头列表
                        let saved_arrows: Vec<_> = state
                            .arrow_objects
                            .iter()
                            .map(|obj| slint_generatedAppWindow::ArrowObject {
                                start_x: obj.start_x,
                                start_y: obj.start_y,
                                end_x: obj.end_x,
                                end_y: obj.end_y,
                                selected: obj.selected,
                            })
                            .collect();
                        app.set_saved_arrows(saved_arrows.as_slice().into());

                        // 更新当前矩形对象状态
                        if let Some(ref rect_obj) = state.current_rectangle_object {
                            app.set_show_current_rectangle(true);
                            app.set_is_drawing_current_rectangle(state.is_drawing_rectangle);
                            app.set_current_rectangle_x(rect_obj.x);
                            app.set_current_rectangle_y(rect_obj.y);
                            app.set_current_rectangle_width(rect_obj.width);
                            app.set_current_rectangle_height(rect_obj.height);
                            app.set_current_rectangle_selected(rect_obj.selected);
                        } else {
                            app.set_show_current_rectangle(false);
                            app.set_is_drawing_current_rectangle(false);
                        }

                        // 更新当前圆形对象状态
                        if let Some(ref circle_obj) = state.current_circle_object {
                            app.set_show_current_circle(true);
                            app.set_is_drawing_current_circle(state.is_drawing_circle);
                            app.set_current_circle_x(circle_obj.x);
                            app.set_current_circle_y(circle_obj.y);
                            app.set_current_circle_width(circle_obj.width);
                            app.set_current_circle_height(circle_obj.height);
                            app.set_current_circle_selected(circle_obj.selected);
                        } else {
                            app.set_show_current_circle(false);
                            app.set_is_drawing_current_circle(false);
                        }

                        // 更新当前箭头对象状态
                        if let Some(ref arrow_obj) = state.current_arrow_object {
                            app.set_show_current_arrow(true);
                            app.set_is_drawing_current_arrow(state.is_drawing_arrow);
                            app.set_current_arrow_start_x(arrow_obj.start_x);
                            app.set_current_arrow_start_y(arrow_obj.start_y);
                            app.set_current_arrow_end_x(arrow_obj.end_x);
                            app.set_current_arrow_end_y(arrow_obj.end_y);
                            app.set_current_arrow_selected(arrow_obj.selected);
                        } else {
                            app.set_show_current_arrow(false);
                            app.set_is_drawing_current_arrow(false);
                        }
                    }
                });

                // 修复：处理工具栏点击 - 恢复完整逻辑
                let app_weak = app.as_weak();
                let app_state_clone = app_state.clone();
                app.on_toolbar_clicked(move |icon_name| {
                    if let Some(app) = app_weak.upgrade() {
                        let mut state = app_state_clone.borrow_mut();

                        let (area_option, should_cancel) = state.handle_toolbar_click(&icon_name);

                        if should_cancel {
                            // 取消按钮被点击
                            app.invoke_cancel_capture();
                        } else if let Some(area) = area_option {
                            // 确认、复制或下载按钮被点击，且有有效选区
                            app.invoke_selection_complete(area);
                        }

                        app.set_selected_icon(state.selected_icon.clone().into());
                    }
                });

                // 处理选区完成
                let background_data_clone = background_data.clone();
                app.on_selection_complete(move |area| {
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

                    std::process::exit(0);
                });

                // 处理取消截图
                app.on_cancel_capture(move || {
                    println!("取消截图");
                    std::process::exit(0);
                });

                // 调试日志
                app.on_debug_log(move |message| {
                    println!("Debug: {}", message);
                });

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

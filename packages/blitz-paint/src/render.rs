mod background;
mod box_shadow;
mod form_controls;

use std::any::Any;
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use crate::color::{Color, ToColorColor};
use crate::debug_overlay::render_debug_overlay;
use crate::kurbo_css::{CssBox, Edge, NonUniformRoundedRectRadii};
use crate::layers::maybe_with_layer;
use crate::sizing::compute_object_fit;
use anyrender::{CustomPaint, Paint, PaintScene};
use blitz_dom::node::{
    ListItemLayout, ListItemLayoutPosition, Marker, NodeData, RasterImageData, TextInputData,
    TextNodeData,
};
use blitz_dom::{BaseDocument, ElementData, Node, local_name};
use blitz_traits::devtools::DevtoolSettings;

use cssparser::{Parser, ParserInput, Token};
use euclid::Transform3D;
use style::values::computed::BorderCornerRadius;
use style::values::computed::length_percentage::Unpacked;
use style::values::generics::basic_shape::{ArcSize, ArcSweep, CoordinatePair};
use style::values::specified::percentage::ToPercentage;
use style::{
    dom::TElement,
    properties::{
        ComputedValues, generated::longhands::visibility::computed_value::T as StyloVisibility,
        style_structs::Font,
    },
    values::{
        computed::{CSSPixelLength, Overflow},
        generics::basic_shape::{GenericPathOrShapeFunction, GenericShapeCommand},
        specified::{BorderStyle, OutlineStyle, image::ImageRendering},
    },
};

use kurbo::{self, Affine, BezPath, Cap, Insets, Point, Rect, Shape, Stroke, Vec2};
use peniko::{self, Fill, ImageData, ImageSampler};
use style::values::computed::{
    angle::Angle,
    length_percentage::{LengthPercentage, NonNegativeLengthPercentage},
    position::Position,
    url::ComputedUrl,
};
use style::values::generics::{
    basic_shape::{
        CommandEndPoint, GenericBasicShape, GenericClipPath, InsetRect, ShapeBox, ShapeGeometryBox,
        ShapeRadius,
    },
    color::GenericColor,
    position::{GenericPosition, GenericPositionOrAuto},
};
use taffy::Layout;

/**
 * The stylo draw command in GenericShapeCommand::Arc uses endpoint parametrization
 * whereas the kurbo::Arc expects the centerpoint parametrization as described here:
 * https://www.w3.org/TR/SVG/implnote.html#ArcImplementationNotes
 *
 * This function implements the conversion of parameters from endpoint to center
 * parametrization as described in the section B.2.4 in the same document.
 *
 * NOTE: Using Kurbo's `SvgArc` type which has the same parameterization
 * as Stylo. However, `SvgArc` is not available in kurbo 0.12.0. If it becomes available in
 * a future version, this conversion could be simplified by using `SvgArc::new()` followed by
 * `SvgArc::to_arc()` to convert to the regular `Arc` type.
 */
fn stylo_to_kurbo_arc(
    start: Point,
    end: Point,
    radii: Vec2,
    arc_sweep: ArcSweep,
    arc_size: ArcSize,
    x_axis_rotation: f64,
) -> kurbo::Arc {
    let rx = radii.x;
    let ry = radii.y;
    let sweep = matches!(arc_sweep, ArcSweep::Cw);
    let large_arc = matches!(arc_size, ArcSize::Large);
    let phi = x_axis_rotation.to_radians();

    // Step1: Compute (x1', y1')
    let half_del_x = (start.x - end.x) / 2.0;
    let half_del_y = (start.y - end.y) / 2.0;
    let x1_prime = phi.cos() * half_del_x + phi.sin() * half_del_y;
    let y1_prime = -phi.sin() * half_del_x + phi.cos() * half_del_y;

    // Step2: Compute (cx', cy')
    let mut coeff = (((rx * ry).powf(2.0) - (rx * y1_prime).powf(2.0) - (ry * x1_prime).powf(2.0))
        / ((rx * y1_prime).powf(2.0) + (ry * x1_prime).powf(2.0)))
    .sqrt();
    if sweep == large_arc {
        coeff = -coeff;
    }

    let cx_prime = coeff * rx * y1_prime / ry;
    let cy_prime = -coeff * ry * x1_prime / rx;

    // Step3: Compute cx and cy
    let mean_x = (start.x + end.x) / 2.0;
    let mean_y = (start.y + end.y) / 2.0;
    let cx = phi.cos() * cx_prime - phi.sin() * cy_prime + mean_x;
    let cy = phi.sin() * cx_prime + phi.cos() * cy_prime + mean_y;

    // Step4: Compute theta1 and delTheta
    let u = Vec2::new(1.0, 0.0);
    let v = Vec2::new((x1_prime - cx_prime) / rx, (y1_prime - cy_prime) / ry);
    let theta1 = angle(u, v);

    let u = v;
    let v = Vec2::new((-x1_prime - cx_prime) / rx, -y1_prime - cy_prime / ry);
    let angle_degree = angle(u, v);

    let del_theta = if sweep && angle_degree > 0.0 {
        angle_degree - 360.0
    } else if !sweep && angle_degree < 0.0 {
        angle_degree + 360.0
    } else {
        angle_degree
    };

    kurbo::Arc::new(
        Point { x: cx, y: cy },
        Vec2 { x: rx, y: ry },
        theta1,
        del_theta,
        x_axis_rotation,
    )
}

pub fn angle(u: Vec2, v: Vec2) -> f64 {
    let sign = (u.x * v.y - u.y * v.x).signum();
    sign * (u.dot(v) / (u.length() * v.length())).acos()
}

fn commands_to_bez_path(cmds: &[GenericShapeCommand<f32, f32>]) -> BezPath {
    let mut path = BezPath::new();
    let mut current_point = Point::new(0.0, 0.0);
    let mut subpath_start = Point::new(0.0, 0.0);
    let mut has_started = false;
    let mut last_quad_ctrl: Option<Point> = None;
    let mut last_cubic_ctrl: Option<Point> = None;

    for cmd in cmds {
        match cmd {
            GenericShapeCommand::Move { point } => {
                let target = resolve_endpoint_absolute(point, current_point);
                path.move_to(target);
                current_point = target;
                subpath_start = target;
                has_started = true;
                last_quad_ctrl = None;
                last_cubic_ctrl = None;
            }
            GenericShapeCommand::Line { point } => {
                let target = resolve_endpoint_absolute(point, current_point);
                ensure_path_started(&mut path, &mut has_started, current_point);
                path.line_to(target);
                current_point = target;
                last_quad_ctrl = None;
                last_cubic_ctrl = None;
            }
            GenericShapeCommand::HLine { by_to, x } => {
                let target = if by_to.is_abs() {
                    Point {
                        x: *x as f64,
                        y: current_point.y,
                    }
                } else {
                    Point {
                        x: current_point.x + *x as f64,
                        y: current_point.y,
                    }
                };
                ensure_path_started(&mut path, &mut has_started, current_point);
                path.line_to(target);
                current_point = target;
                last_quad_ctrl = None;
                last_cubic_ctrl = None;
            }
            GenericShapeCommand::VLine { by_to, y } => {
                let target = if by_to.is_abs() {
                    Point {
                        x: current_point.x,
                        y: *y as f64,
                    }
                } else {
                    Point {
                        x: current_point.x,
                        y: current_point.y + *y as f64,
                    }
                };
                ensure_path_started(&mut path, &mut has_started, current_point);
                path.line_to(target);
                current_point = target;
                last_quad_ctrl = None;
                last_cubic_ctrl = None;
            }
            GenericShapeCommand::Arc {
                point,
                radii,
                arc_sweep,
                arc_size,
                rotate,
            } => {
                let target = resolve_endpoint_absolute(point, current_point);
                let arc = stylo_to_kurbo_arc(
                    current_point,
                    target,
                    Vec2 {
                        x: radii.x as f64,
                        y: radii.y as f64,
                    },
                    *arc_sweep,
                    *arc_size,
                    *rotate as f64,
                );
                ensure_path_started(&mut path, &mut has_started, current_point);
                for el in arc.to_path(1e-3) {
                    match el {
                        kurbo::PathEl::MoveTo(_) => {}
                        _ => path.push(el),
                    }
                }
                current_point = target;
                last_quad_ctrl = None;
                last_cubic_ctrl = None;
            }
            GenericShapeCommand::QuadCurve { point, control1 } => {
                let control = coordinate_pair_to_point(control1);
                let target = resolve_endpoint_absolute(point, current_point);
                ensure_path_started(&mut path, &mut has_started, current_point);
                path.quad_to(control, target);
                current_point = target;
                last_quad_ctrl = Some(control);
                last_cubic_ctrl = None;
            }
            GenericShapeCommand::CubicCurve {
                point,
                control1,
                control2,
            } => {
                let control_one = coordinate_pair_to_point(control1);
                let control_two = coordinate_pair_to_point(control2);
                let target = resolve_endpoint_absolute(point, current_point);
                ensure_path_started(&mut path, &mut has_started, current_point);
                path.curve_to(control_one, control_two, target);
                current_point = target;
                last_cubic_ctrl = Some(control_two);
                last_quad_ctrl = None;
            }
            GenericShapeCommand::SmoothQuad { point } => {
                let control = last_quad_ctrl
                    .map(|ctrl| reflect_point(ctrl, current_point))
                    .unwrap_or(current_point);
                let target = resolve_endpoint_absolute(point, current_point);
                ensure_path_started(&mut path, &mut has_started, current_point);
                path.quad_to(control, target);
                current_point = target;
                last_quad_ctrl = Some(control);
                last_cubic_ctrl = None;
            }
            GenericShapeCommand::SmoothCubic { point, control2 } => {
                let control_one = last_cubic_ctrl
                    .map(|ctrl| reflect_point(ctrl, current_point))
                    .unwrap_or(current_point);
                let control_two = coordinate_pair_to_point(control2);
                let target = resolve_endpoint_absolute(point, current_point);
                ensure_path_started(&mut path, &mut has_started, current_point);
                path.curve_to(control_one, control_two, target);
                current_point = target;
                last_cubic_ctrl = Some(control_two);
                last_quad_ctrl = None;
            }
            GenericShapeCommand::Close => {
                ensure_path_started(&mut path, &mut has_started, current_point);
                path.close_path();
                current_point = subpath_start;
                last_quad_ctrl = None;
                last_cubic_ctrl = None;
            }
        }
    }

    path
}

/// A short-lived struct which holds a bunch of parameters for rendering a scene so
/// that we don't have to pass them down as parameters.
///
/// This struct is created fresh for each frame and dropped after rendering completes.
/// The `clip_path_cache` is populated during rendering and cleared when the struct is dropped.
pub struct BlitzDomPainter<'dom> {
    /// Input parameters (read only) for generating the Scene
    pub(crate) dom: &'dom BaseDocument,
    pub(crate) scale: f64,
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) devtools: DevtoolSettings,
    /// Per-frame cache of computed clip-path BezPaths, keyed by node ID.
    ///
    /// This cache is populated during rendering to avoid recomputing clip paths
    /// for nodes that are referenced multiple times (e.g., via `clip-path: url(#id)`).
    /// The cache is cleared when the `BlitzDomPainter` instance is dropped after each frame.
    pub(crate) clip_path_cache: RefCell<HashMap<usize, Rc<BezPath>>>,
}

impl BlitzDomPainter<'_> {
    fn node_position(&self, node: usize, location: Point) -> (Layout, Point) {
        let layout = self.layout(node);
        let pos = location + Vec2::new(layout.location.x as f64, layout.location.y as f64);
        (layout, pos)
    }

    fn layout(&self, child: usize) -> Layout {
        // self.dom.as_ref().tree()[child].unrounded_layout
        self.dom.as_ref().tree()[child].final_layout
    }

    /// Draw the current tree to current render surface
    /// Eventually we'll want the surface itself to be passed into the render function, along with things like the viewport
    ///
    /// This assumes styles are resolved and layout is complete.
    /// Make sure you do those before trying to render
    pub fn paint_scene(&self, scene: &mut impl PaintScene) {
        // Simply render the document (the root element (note that this is not the same as the root node)))
        scene.reset();
        let viewport_scroll = self.dom.as_ref().viewport_scroll();

        let root_element = self.dom.as_ref().root_element();
        let root_id = root_element.id;
        let bg_width = (self.width as f32).max(root_element.final_layout.size.width);
        let bg_height = (self.height as f32).max(root_element.final_layout.size.height);

        let background_color = {
            let html_color = root_element
                .primary_styles()
                .map(|s| s.clone_background_color())
                .unwrap_or(GenericColor::TRANSPARENT_BLACK);
            if html_color == GenericColor::TRANSPARENT_BLACK {
                root_element
                    .children
                    .iter()
                    .find_map(|id| {
                        self.dom
                            .as_ref()
                            .get_node(*id)
                            .filter(|node| node.data.is_element_with_tag_name(&local_name!("body")))
                    })
                    .and_then(|body| body.primary_styles())
                    .map(|style| {
                        let current_color = style.clone_color();
                        style
                            .clone_background_color()
                            .resolve_to_absolute(&current_color)
                    })
            } else {
                let current_color = root_element.primary_styles().unwrap().clone_color();
                Some(html_color.resolve_to_absolute(&current_color))
            }
        };

        if let Some(bg_color) = background_color {
            let bg_color = bg_color.as_srgb_color();
            let rect = Rect::from_origin_size((0.0, 0.0), (bg_width as f64, bg_height as f64));
            scene.fill(Fill::NonZero, Affine::IDENTITY, bg_color, None, &rect);
        }

        self.render_element(
            scene,
            root_id,
            Point {
                x: -viewport_scroll.x,
                y: -viewport_scroll.y,
            },
        );

        // Render debug overlay
        if self.devtools.highlight_hover {
            if let Some(node_id) = self.dom.as_ref().get_hover_node_id() {
                render_debug_overlay(scene, self.dom, node_id, self.scale);
            }
        }
    }

    /// Renders a node, but is guaranteed that the node is an element
    /// This is because the font_size is calculated from layout resolution and all text is rendered directly here, instead
    /// of a separate text stroking phase.
    ///
    /// In Blitz, text styling gets its attributes from its container element/resolved styles
    /// In other libraries, text gets its attributes from a `text` element - this is not how HTML works.
    ///
    /// Approaching rendering this way guarantees we have all the styles we need when rendering text with not having
    /// to traverse back to the parent for its styles, or needing to pass down styles
    fn render_element(&self, scene: &mut impl PaintScene, node_id: usize, location: Point) {
        let node = &self.dom.as_ref().tree()[node_id];

        // Early return if the element is hidden
        if matches!(node.style.display, taffy::Display::None) {
            return;
        }

        // Only draw elements with a style
        if node.primary_styles().is_none() {
            return;
        }

        // Hide inputs with type=hidden
        // Implemented here rather than using the style engine for performance reasons
        if node.local_name() == "input" && node.attr(local_name!("type")) == Some("hidden") {
            return;
        }

        // Hide elements with a visibility style other than visible
        if node
            .primary_styles()
            .unwrap()
            .get_inherited_box()
            .visibility
            != StyloVisibility::Visible
        {
            return;
        }

        // We can't fully support opacity yet, but we can hide elements with opacity 0
        let opacity = node.primary_styles().unwrap().get_effects().opacity;
        if opacity == 0.0 {
            return;
        }
        let has_opacity = opacity < 1.0;

        // TODO: account for overflow_x vs overflow_y
        let styles = &node.primary_styles().unwrap();
        let overflow_x = styles.get_box().overflow_x;
        let overflow_y = styles.get_box().overflow_y;
        let is_image = node
            .element_data()
            .and_then(|e| e.raster_image_data())
            .is_some();
        let should_clip = is_image
            || !matches!(overflow_x, Overflow::Visible)
            || !matches!(overflow_y, Overflow::Visible);

        // Apply padding/border offset to inline root
        let (layout, box_position) = self.node_position(node_id, location);
        let taffy::Layout {
            size,
            border,
            padding,
            content_size,
            ..
        } = node.final_layout;
        let scaled_pb = (padding + border).map(f64::from);
        let content_position = kurbo::Point {
            x: box_position.x + scaled_pb.left,
            y: box_position.y + scaled_pb.top,
        };
        let content_box_size = kurbo::Size {
            width: (size.width as f64 - scaled_pb.left - scaled_pb.right) * self.scale,
            height: (size.height as f64 - scaled_pb.top - scaled_pb.bottom) * self.scale,
        };

        // Don't render things that are out of view
        let scaled_y = box_position.y * self.scale;
        let scaled_content_height = content_size.height.max(size.height) as f64 * self.scale;
        if scaled_y > self.height as f64 || scaled_y + scaled_content_height < 0.0 {
            return;
        }

        // Optimise zero-area (/very small area) clips by not rendering at all
        let clip_area = content_box_size.width * content_box_size.height;
        if should_clip && clip_area < 0.01 {
            return;
        }

        let mut cx = self.element_cx(node, layout, box_position);

        let clip_path = self.clip_path_from_styles(node_id, node, styles);
        let wants_layer = should_clip || has_opacity || clip_path.is_some();
        let fallback_clip_shape;
        let clip_shape: &BezPath = match clip_path.as_ref() {
            Some(path) => path,
            None => {
                fallback_clip_shape = cx.frame.padding_box_path();
                &fallback_clip_shape
            }
        };

        maybe_with_layer(
            scene,
            wants_layer,
            opacity,
            cx.transform,
            clip_shape,
            |scene| {
                // Draw borders, background, and outline inside the clip-path
                // According to CSS spec, clip-path clips everything including borders
                cx.draw_outline(scene);
                cx.draw_outset_box_shadow(scene);
                cx.draw_background(scene);
                cx.draw_border(scene);
                cx.draw_inset_box_shadow(scene);
                cx.stroke_devtools(scene);

                // Now that background has been drawn, offset pos and cx in order to draw our contents scrolled
                let content_position = Point {
                    x: content_position.x - node.scroll_offset.x,
                    y: content_position.y - node.scroll_offset.y,
                };
                cx.pos = Point {
                    x: cx.pos.x - node.scroll_offset.x,
                    y: cx.pos.y - node.scroll_offset.y,
                };
                cx.transform = cx.transform.then_translate(Vec2 {
                    x: -node.scroll_offset.x,
                    y: -node.scroll_offset.y,
                });
                cx.draw_image(scene);
                #[cfg(feature = "svg")]
                cx.draw_svg(scene);
                cx.draw_canvas(scene);
                cx.draw_input(scene);

                cx.draw_text_input_text(scene, content_position);
                cx.draw_inline_layout(scene, content_position);
                cx.draw_marker(scene, content_position);
                cx.draw_children(scene);
            },
        );
    }

    fn render_node(&self, scene: &mut impl PaintScene, node_id: usize, location: Point) {
        let node = &self.dom.as_ref().tree()[node_id];

        match &node.data {
            NodeData::Element(_) | NodeData::AnonymousBlock(_) => {
                self.render_element(scene, node_id, location)
            }
            NodeData::Text(TextNodeData { .. }) => {
                // Text nodes should never be rendered directly
                // (they should always be rendered as part of an inline layout)
                // unreachable!()
            }
            NodeData::Document => {}
            // NodeData::Doctype => {}
            NodeData::Comment => {} // NodeData::ProcessingInstruction { .. } => {}
        }
    }

    fn clip_path_from_styles(
        &self,
        node_id: usize,
        node: &Node,
        styles: &ComputedValues,
    ) -> Option<Rc<BezPath>> {
        let mut visited = HashSet::new();
        let canonical_frame = create_css_rect(styles, &node.final_layout, 1.0);
        self.clip_path_from_styles_inner(node_id, &canonical_frame, styles, &mut visited)
            .map(|path| {
                if (self.scale - 1.0).abs() < f64::EPSILON {
                    path
                } else {
                    let mut scaled = (*path).clone();
                    scaled.apply_affine(Affine::scale(self.scale));
                    Rc::new(scaled)
                }
            })
    }

    fn clip_path_from_styles_inner(
        &self,
        node_id: usize,
        frame: &CssBox,
        styles: &ComputedValues,
        visited: &mut HashSet<usize>,
    ) -> Option<Rc<BezPath>> {
        let node = &self.dom.as_ref().tree()[node_id];
        if let Some(cached) = self.clip_path_cache.borrow().get(&node_id) {
            node.set_cached_clip_path(Some(cached.clone()));
            return Some(cached.clone());
        }

        if !visited.insert(node_id) {
            return None;
        }

        type StyloBasicShape = GenericBasicShape<
            Angle,
            Position,
            LengthPercentage,
            NonNegativeLengthPercentage,
            InsetRect<LengthPercentage, NonNegativeLengthPercentage>,
        >;
        type StyloClipPath = GenericClipPath<StyloBasicShape, ComputedUrl>;
        let stylo_clip_path: StyloClipPath = styles.clone_clip_path();

        let reference_rect = frame.border_box;
        let shape_margin = resolve_shape_margin_for_node(node, reference_rect);

        let path = match stylo_clip_path {
            GenericClipPath::None => None,
            GenericClipPath::Url(url) => self.clip_path_from_url(node_id, url, visited),
            GenericClipPath::Box(geometry_box) => Some(Rc::new(
                self.clip_path_for_geometry_box(frame, geometry_box),
            )),
            GenericClipPath::Shape(basic_shape, geometry_box) => {
                let reference_rect = self.reference_rect_for_geometry_box(frame, geometry_box);
                self.basic_shape_to_path(frame, *basic_shape, reference_rect, shape_margin)
                    .map(Rc::new)
            }
        };

        visited.remove(&node_id);

        match path {
            Some(path) => {
                node.set_cached_clip_path(Some(path.clone()));
                self.clip_path_cache
                    .borrow_mut()
                    .insert(node_id, path.clone());
                Some(path)
            }
            None => {
                node.set_cached_clip_path(None);
                None
            }
        }
    }

    fn clip_path_from_url(
        &self,
        _node_id: usize,
        url: ComputedUrl,
        visited: &mut HashSet<usize>,
    ) -> Option<Rc<BezPath>> {
        let resolved = url.url()?;
        let fragment = resolved.fragment()?;
        let referenced_id = self.dom.get_element_by_id(fragment)?;
        if visited.contains(&referenced_id) {
            return None;
        }
        let referenced_node = &self.dom.as_ref().tree()[referenced_id];
        let styles = referenced_node.primary_styles()?;
        let frame = create_css_rect(&*styles, &referenced_node.final_layout, 1.0);
        self.clip_path_from_styles_inner(referenced_id, &frame, &*styles, visited)
    }

    fn clip_path_for_geometry_box(
        &self,
        frame: &CssBox,
        geometry_box: ShapeGeometryBox,
    ) -> BezPath {
        match geometry_box {
            ShapeGeometryBox::ShapeBox(shape_box) => match shape_box {
                ShapeBox::BorderBox => frame.border_box_path(),
                ShapeBox::PaddingBox => frame.padding_box_path(),
                ShapeBox::ContentBox => frame.content_box_path(),
                ShapeBox::MarginBox => frame.margin_box_path(),
            },
            // For now, map SVG-specific boxes (fill-box, stroke-box, view-box) and
            // element-dependent defaults to the border box for HTML elements. This
            // keeps behavior aligned with the spec default while we lack SVG layout.
            ShapeGeometryBox::FillBox
            | ShapeGeometryBox::StrokeBox
            | ShapeGeometryBox::ViewBox
            | ShapeGeometryBox::ElementDependent => frame.border_box_path(),
        }
    }

    fn reference_rect_for_geometry_box(
        &self,
        frame: &CssBox,
        geometry_box: ShapeGeometryBox,
    ) -> Rect {
        match geometry_box {
            ShapeGeometryBox::ShapeBox(shape_box) => match shape_box {
                ShapeBox::BorderBox => frame.border_box,
                ShapeBox::PaddingBox => frame.padding_box,
                ShapeBox::ContentBox => frame.content_box,
                ShapeBox::MarginBox => frame.margin_box,
            },
            ShapeGeometryBox::FillBox
            | ShapeGeometryBox::StrokeBox
            | ShapeGeometryBox::ViewBox
            | ShapeGeometryBox::ElementDependent => frame.border_box,
        }
    }

    fn basic_shape_to_path(
        &self,
        frame: &CssBox,
        shape: GenericBasicShape<
            Angle,
            Position,
            LengthPercentage,
            NonNegativeLengthPercentage,
            InsetRect<LengthPercentage, NonNegativeLengthPercentage>,
        >,
        reference_rect: Rect,
        shape_margin: f64,
    ) -> Option<BezPath> {
        let origin = reference_rect.origin();
        let origin_x = origin.x;
        let origin_y = origin.y;
        let box_width = reference_rect.width();
        let box_height = reference_rect.height();

        match shape {
            GenericBasicShape::Circle(circle) => {
                let center = resolve_shape_position(circle.position, reference_rect);
                let radius =
                    resolve_shape_radius(&circle.radius, center, reference_rect, box_width);
                let total_radius = radius + shape_margin;

                // Create the path even if radius is zero/negative - we'll check area later
                Some(frame.circle_path(center, total_radius.max(0.0)))
            }
            GenericBasicShape::Ellipse(ellipse) => {
                let center = resolve_shape_position(ellipse.position, reference_rect);
                let radius_x =
                    resolve_shape_radius(&ellipse.semiaxis_x, center, reference_rect, box_width)
                        + shape_margin;
                let radius_y =
                    resolve_shape_radius(&ellipse.semiaxis_y, center, reference_rect, box_height)
                        + shape_margin;

                // Create the path even if radii are zero/negative - we'll check area later
                Some(frame.ellipse_path(
                    center,
                    Vec2 {
                        x: radius_x.max(0.0),
                        y: radius_y.max(0.0),
                    },
                ))
            }
            GenericBasicShape::Rect(rect) => {
                let top = resolve_length_percentage_value(&rect.rect.0, box_height);
                let right = resolve_length_percentage_value(&rect.rect.1, box_width);
                let bottom = resolve_length_percentage_value(&rect.rect.2, box_height);
                let left = resolve_length_percentage_value(&rect.rect.3, box_width);

                let x0_raw = origin_x + left - shape_margin;
                let y0_raw = origin_y + top - shape_margin;
                let x1_raw = origin_x + box_width - right + shape_margin;
                let y1_raw = origin_y + box_height - bottom + shape_margin;

                // Ensure valid rect bounds (swap if needed)
                let x0 = x0_raw.min(x1_raw);
                let y0 = y0_raw.min(y1_raw);
                let x1 = x0_raw.max(x1_raw);
                let y1 = y0_raw.max(y1_raw);

                let r_top_left = resolve_non_negative_length_percentage_value(
                    rect.round.top_left.0.width(),
                    box_width,
                );
                let r_top_right = resolve_non_negative_length_percentage_value(
                    rect.round.top_right.0.width(),
                    box_width,
                );
                let r_bottom_right = resolve_non_negative_length_percentage_value(
                    rect.round.bottom_right.0.width(),
                    box_width,
                );
                let r_bottom_left = resolve_non_negative_length_percentage_value(
                    rect.round.bottom_left.0.width(),
                    box_width,
                );

                Some(frame.rect_path(
                    x0,
                    y0,
                    x1,
                    y1,
                    (r_top_left, r_top_right, r_bottom_right, r_bottom_left),
                ))
            }
            GenericBasicShape::Polygon(polygon) => {
                let bounding_box_center = Point {
                    x: origin_x + box_width / 2.0,
                    y: origin_y + box_height / 2.0,
                };

                let points: Vec<Point> = polygon
                    .coordinates
                    .iter()
                    .map(|point| {
                        let base_point = Point {
                            x: origin_x + resolve_length_percentage_value(&point.0, box_width),
                            y: origin_y + resolve_length_percentage_value(&point.1, box_height),
                        };
                        expand_polygon_point(base_point, bounding_box_center, shape_margin)
                    })
                    .collect();

                // Create the path even if degenerate - we'll check area later
                Some(frame.polygon_path(&points))
            }
            GenericBasicShape::PathOrShape(path) => {
                let path = match path {
                    // CSS path() authoring is typically 0..1 relative to the reference box.
                    // Scale to the reference box size, then translate to its origin.
                    GenericPathOrShapeFunction::Path(p) => {
                        let mut pth = commands_to_bez_path(p.commands());
                        pth.apply_affine(
                            Affine::scale_non_uniform(box_width, box_height)
                                .then_translate(Vec2::new(origin_x, origin_y)),
                        );
                        pth
                    }
                    // CSS shape() commands are already resolved into absolute lengths/percentages.
                    GenericPathOrShapeFunction::Shape(s) => {
                        let cmds =
                            convert_shape_commands_to_absolute(s.commands(), box_width, box_height);
                        let mut pth = commands_to_bez_path(&cmds);
                        pth.apply_affine(Affine::translate((origin_x, origin_y)));
                        pth
                    }
                };

                Some(path)
            }
        }
    }

    fn element_cx<'w>(
        &'w self,
        node: &'w Node,
        layout: Layout,
        box_position: Point,
    ) -> ElementCx<'w> {
        let style = node
            .stylo_element_data
            .borrow()
            .as_ref()
            .map(|element_data| element_data.styles.primary().clone())
            .unwrap_or(
                ComputedValues::initial_values_with_font_override(Font::initial_values()).to_arc(),
            );

        let scale = self.scale;

        // todo: maybe cache this so we don't need to constantly be figuring it out
        // It is quite a bit of math to calculate during render/traverse
        // Also! we can cache the bezpaths themselves, saving us a bunch of work
        let frame = create_css_rect(&style, &layout, scale);

        // the bezpaths for every element are (potentially) cached (not yet, tbd)
        // By performing the transform, we prevent the cache from becoming invalid when the page shifts around
        let mut transform = Affine::translate(box_position.to_vec2() * scale);

        // Reference box for resolve percentage transforms
        let reference_box = euclid::Rect::new(
            euclid::Point2D::new(CSSPixelLength::new(0.0), CSSPixelLength::new(0.0)),
            euclid::Size2D::new(
                CSSPixelLength::new(frame.border_box.width() as f32),
                CSSPixelLength::new(frame.border_box.height() as f32),
            ),
        );

        // Apply CSS transform property (where transforms are 2d)
        //
        // TODO: Handle hit testing correctly for transformed nodes
        // TODO: Implement nested transforms
        let (t, has_3d) = &style
            .get_box()
            .transform
            .to_transform_3d_matrix(Some(&reference_box))
            .unwrap_or((Transform3D::default(), false));
        if !has_3d {
            // See: https://drafts.csswg.org/css-transforms-2/#two-dimensional-subset
            // And https://docs.rs/kurbo/latest/kurbo/struct.Affine.html#method.new
            let kurbo_transform =
                Affine::new([t.m11, t.m12, t.m21, t.m22, t.m41, t.m42].map(|v| v as f64));

            // Apply the transform origin by:
            //   - Translating by the origin offset
            //   - Applying our transform
            //   - Translating by the inverse of the origin offset
            let transform_origin = &style.get_box().transform_origin;
            let origin_translation = Affine::translate(Vec2 {
                x: transform_origin
                    .horizontal
                    .resolve(CSSPixelLength::new(frame.border_box.width() as f32))
                    .px() as f64,
                y: transform_origin
                    .vertical
                    .resolve(CSSPixelLength::new(frame.border_box.height() as f32))
                    .px() as f64,
            });
            let kurbo_transform =
                origin_translation * kurbo_transform * origin_translation.inverse();

            transform *= kurbo_transform;
        }

        let element = node.element_data().unwrap();

        ElementCx {
            context: self,
            frame,
            scale,
            style,
            pos: box_position,
            node,
            element,
            transform,
            #[cfg(feature = "svg")]
            svg: element.svg_data(),
            text_input: element.text_input_data(),
            list_item: element.list_item_data.as_deref(),
            devtools: &self.devtools,
        }
    }
}

fn to_image_quality(image_rendering: ImageRendering) -> peniko::ImageQuality {
    match image_rendering {
        ImageRendering::Auto => peniko::ImageQuality::Medium,
        ImageRendering::CrispEdges => peniko::ImageQuality::Low,
        ImageRendering::Pixelated => peniko::ImageQuality::Low,
    }
}

/// Ensure that the `resized_image` field has a correctly sized image
fn to_peniko_image(image: &RasterImageData, quality: peniko::ImageQuality) -> peniko::ImageBrush {
    peniko::ImageBrush {
        image: ImageData {
            data: image.data.clone(),
            format: peniko::ImageFormat::Rgba8,
            width: image.width,
            height: image.height,
            alpha_type: peniko::ImageAlphaType::Alpha,
        },
        sampler: ImageSampler {
            x_extend: peniko::Extend::Repeat,
            y_extend: peniko::Extend::Repeat,
            quality,
            alpha: 1.0,
        },
    }
}

/// A context of loaded and hot data to draw the element from
struct ElementCx<'a> {
    context: &'a BlitzDomPainter<'a>,
    frame: CssBox,
    style: style::servo_arc::Arc<ComputedValues>,
    pos: Point,
    scale: f64,
    node: &'a Node,
    element: &'a ElementData,
    transform: Affine,
    #[cfg(feature = "svg")]
    svg: Option<&'a usvg::Tree>,
    text_input: Option<&'a TextInputData>,
    list_item: Option<&'a ListItemLayout>,
    devtools: &'a DevtoolSettings,
}

/// Converts parley BoundingBox into peniko Rect
fn convert_rect(rect: &parley::BoundingBox) -> kurbo::Rect {
    peniko::kurbo::Rect::new(rect.x0, rect.y0, rect.x1, rect.y1)
}

impl ElementCx<'_> {
    fn draw_inline_layout(&self, scene: &mut impl PaintScene, pos: Point) {
        if self.node.flags.is_inline_root() {
            let text_layout = self.element
                .inline_layout_data
                .as_ref()
                .unwrap_or_else(|| {
                    panic!("Tried to render node marked as inline root that does not have an inline layout: {:?}", self.node);
                });

            // Render text
            crate::text::stroke_text(
                self.scale,
                scene,
                text_layout.layout.lines(),
                self.context.dom,
                pos,
            );
        }
    }

    fn draw_text_input_text(&self, scene: &mut impl PaintScene, pos: Point) {
        // Render the text in text inputs
        if let Some(input_data) = self.text_input {
            let transform = Affine::translate((pos.x * self.scale, pos.y * self.scale));

            if self.node.is_focussed() {
                // Render selection/caret
                for (rect, _line_idx) in input_data.editor.selection_geometry().iter() {
                    scene.fill(
                        Fill::NonZero,
                        transform,
                        color::palette::css::STEEL_BLUE,
                        None,
                        &convert_rect(rect),
                    );
                }
                if let Some(cursor) = input_data.editor.cursor_geometry(1.5) {
                    // TODO: Use the `caret-color` attribute here if present.
                    let color = self.style.get_inherited_text().color;

                    scene.fill(
                        Fill::NonZero,
                        transform,
                        color.as_srgb_color(),
                        None,
                        &convert_rect(&cursor),
                    );
                };
            }

            // Render text
            crate::text::stroke_text(
                self.scale,
                scene,
                input_data.editor.try_layout().unwrap().lines(),
                self.context.dom,
                pos,
            );
        }
    }

    fn draw_marker(&self, scene: &mut impl PaintScene, pos: Point) {
        if let Some(ListItemLayout {
            marker,
            position: ListItemLayoutPosition::Outside(layout),
        }) = self.list_item
        {
            // Right align and pad the bullet when rendering outside
            let x_padding = match marker {
                Marker::Char(_) => 8.0,
                Marker::String(_) => 0.0,
            };
            let x_offset = -(layout.full_width() / layout.scale() + x_padding);

            // Align the marker with the baseline of the first line of text in the list item
            let y_offset = if let Some(first_text_line) = &self
                .element
                .inline_layout_data
                .as_ref()
                .and_then(|text_layout| text_layout.layout.lines().next())
            {
                (first_text_line.metrics().baseline
                    - layout.lines().next().unwrap().metrics().baseline)
                    / layout.scale()
            } else {
                0.0
            };

            let pos = Point {
                x: pos.x + x_offset as f64,
                y: pos.y + y_offset as f64,
            };

            crate::text::stroke_text(self.scale, scene, layout.lines(), self.context.dom, pos);
        }
    }

    fn draw_children(&self, scene: &mut impl PaintScene) {
        // Negative z_index hoisted nodes
        if let Some(hoisted) = &self.node.stacking_context {
            for hoisted_child in hoisted.neg_z_hoisted_children() {
                let pos = kurbo::Point {
                    x: self.pos.x + hoisted_child.position.x as f64,
                    y: self.pos.y + hoisted_child.position.y as f64,
                };
                self.render_node(scene, hoisted_child.node_id, pos);
            }
        }

        // Regular children
        if let Some(children) = &*self.node.paint_children.borrow() {
            for child_id in children {
                self.render_node(scene, *child_id, self.pos);
            }
        }

        // Positive z_index hoisted nodes
        if let Some(hoisted) = &self.node.stacking_context {
            for hoisted_child in hoisted.pos_z_hoisted_children() {
                let pos = kurbo::Point {
                    x: self.pos.x + hoisted_child.position.x as f64,
                    y: self.pos.y + hoisted_child.position.y as f64,
                };
                self.render_node(scene, hoisted_child.node_id, pos);
            }
        }
    }

    #[cfg(feature = "svg")]
    fn draw_svg(&self, scene: &mut impl PaintScene) {
        use style::properties::generated::longhands::object_fit::computed_value::T as ObjectFit;

        let Some(svg) = self.svg else {
            return;
        };

        let width = self.frame.content_box.width() as u32;
        let height = self.frame.content_box.height() as u32;
        let svg_size = svg.size();

        let x = self.frame.content_box.origin().x;
        let y = self.frame.content_box.origin().y;

        // let object_fit = self.style.clone_object_fit();
        let object_position = self.style.clone_object_position();

        // Apply object-fit algorithm
        let container_size = taffy::Size {
            width: width as f32,
            height: height as f32,
        };
        let object_size = taffy::Size {
            width: svg_size.width(),
            height: svg_size.height(),
        };
        let paint_size = compute_object_fit(container_size, Some(object_size), ObjectFit::Contain);

        // Compute object-position
        let x_offset = object_position.horizontal.resolve(
            CSSPixelLength::new(container_size.width - paint_size.width) / self.scale as f32,
        ) * self.scale as f32;
        let y_offset = object_position.vertical.resolve(
            CSSPixelLength::new(container_size.height - paint_size.height) / self.scale as f32,
        ) * self.scale as f32;
        let x = x + x_offset.px() as f64;
        let y = y + y_offset.px() as f64;

        let x_scale = paint_size.width as f64 / object_size.width as f64;
        let y_scale = paint_size.height as f64 / object_size.height as f64;

        let transform =
            Affine::translate((self.pos.x * self.scale + x, self.pos.y * self.scale + y))
                .pre_scale_non_uniform(x_scale, y_scale);

        anyrender_svg::render_svg_tree(scene, svg, transform);
    }

    fn draw_image(&self, scene: &mut impl PaintScene) {
        if let Some(image) = self.element.raster_image_data() {
            let width = self.frame.content_box.width() as u32;
            let height = self.frame.content_box.height() as u32;
            let x = self.frame.content_box.origin().x;
            let y = self.frame.content_box.origin().y;

            let object_fit = self.style.clone_object_fit();
            let object_position = self.style.clone_object_position();
            let image_rendering = self.style.clone_image_rendering();
            let quality = to_image_quality(image_rendering);

            // Apply object-fit algorithm
            let container_size = taffy::Size {
                width: width as f32,
                height: height as f32,
            };
            let object_size = taffy::Size {
                width: image.width as f32,
                height: image.height as f32,
            };
            let paint_size = compute_object_fit(container_size, Some(object_size), object_fit);

            // Compute object-position
            let x_offset = object_position.horizontal.resolve(
                CSSPixelLength::new(container_size.width - paint_size.width) / self.scale as f32,
            ) * self.scale as f32;
            let y_offset = object_position.vertical.resolve(
                CSSPixelLength::new(container_size.height - paint_size.height) / self.scale as f32,
            ) * self.scale as f32;
            let x = x + x_offset.px() as f64;
            let y = y + y_offset.px() as f64;

            let x_scale = paint_size.width as f64 / object_size.width as f64;
            let y_scale = paint_size.height as f64 / object_size.height as f64;
            let transform = self
                .transform
                .pre_scale_non_uniform(x_scale, y_scale)
                .then_translate(Vec2 { x, y });

            scene.draw_image(to_peniko_image(image, quality).as_ref(), transform);
        }
    }

    fn draw_canvas(&self, scene: &mut impl PaintScene) {
        if let Some(custom_paint_source) = self.element.canvas_data() {
            let width = self.frame.content_box.width() as u32;
            let height = self.frame.content_box.height() as u32;
            let x = self.frame.content_box.origin().x;
            let y = self.frame.content_box.origin().y;

            let transform = self.transform.then_translate(Vec2 { x, y });

            scene.fill(
                Fill::NonZero,
                transform,
                // TODO: replace `Arc<dyn Any>` with `CustomPaint` in API?
                Paint::Custom(&CustomPaint {
                    source_id: custom_paint_source.custom_paint_source_id,
                    width,
                    height,
                    scale: self.scale,
                } as &(dyn Any + Send + Sync)),
                None,
                &Rect::from_origin_size((0.0, 0.0), (width as f64, height as f64)),
            );
        }
    }

    fn stroke_devtools(&self, scene: &mut impl PaintScene) {
        if self.devtools.show_layout {
            let shape = &self.frame.border_box;
            let stroke = Stroke::new(self.scale);

            let stroke_color = match self.node.style.display {
                taffy::Display::Block => Color::new([1.0, 0.0, 0.0, 1.0]),
                taffy::Display::Flex => Color::new([0.0, 1.0, 0.0, 1.0]),
                taffy::Display::Grid => Color::new([0.0, 0.0, 1.0, 1.0]),
                taffy::Display::None => Color::new([0.0, 0.0, 1.0, 1.0]),
            };

            scene.stroke(&stroke, self.transform, stroke_color, None, &shape);
        }
    }

    /// Stroke a border
    ///
    /// The border-style property specifies what kind of border to display.
    ///
    /// The following values are allowed:
    /// ✅ dotted - Defines a dotted border
    /// ✅ dashed - Defines a dashed border
    /// ✅ solid - Defines a solid border
    /// ❌ double - Defines a double border
    /// ❌ groove - Defines a 3D grooved border.
    /// ❌ ridge - Defines a 3D ridged border.
    /// ❌ inset - Defines a 3D inset border.
    /// ❌ outset - Defines a 3D outset border.
    /// ✅ none - Defines no border
    /// ✅ hidden - Defines a hidden border
    ///
    /// The border-style property can have from one to four values (for the top border, right border, bottom border, and the left border).
    fn draw_border(&self, sb: &mut impl PaintScene) {
        for edge in [Edge::Top, Edge::Right, Edge::Bottom, Edge::Left] {
            self.draw_border_edge(sb, edge);
        }
    }

    /// The border-style property specifies what kind of border to display.
    ///
    /// [Border](https://www.w3schools.com/css/css_border.asp)
    ///
    /// The following values are allowed:
    /// - ✅ dotted: Defines a dotted border
    /// - ✅ dashed: Defines a dashed border
    /// - ✅ solid: Defines a solid border
    /// - ❌ double: Defines a double border
    /// - ❌ groove: Defines a 3D grooved border*
    /// - ❌ ridge: Defines a 3D ridged border*
    /// - ❌ inset: Defines a 3D inset border*
    /// - ❌ outset: Defines a 3D outset border*
    /// - ✅ none: Defines no border
    /// - ✅ hidden: Defines a hidden border
    ///
    /// [*] The effect depends on the border-color value
    fn draw_border_edge(&self, sb: &mut impl PaintScene, edge: Edge) {
        let style = &*self.style;
        let border = style.get_border();
        let path = self.frame.border_edge_shape(edge);

        let current_color = style.clone_color();
        let color = match edge {
            Edge::Top => border
                .border_top_color
                .resolve_to_absolute(&current_color)
                .as_srgb_color(),
            Edge::Right => border
                .border_right_color
                .resolve_to_absolute(&current_color)
                .as_srgb_color(),
            Edge::Bottom => border
                .border_bottom_color
                .resolve_to_absolute(&current_color)
                .as_srgb_color(),
            Edge::Left => border
                .border_left_color
                .resolve_to_absolute(&current_color)
                .as_srgb_color(),
        };

        let border_style = match edge {
            Edge::Top => border.border_top_style,
            Edge::Right => border.border_right_style,
            Edge::Bottom => border.border_bottom_style,
            Edge::Left => border.border_left_style,
        };

        let border_width = match edge {
            Edge::Top => self.frame.border_width.y0,
            Edge::Right => self.frame.border_width.x1,
            Edge::Bottom => self.frame.border_width.y1,
            Edge::Left => self.frame.border_width.x0,
        };

        let alpha = color.components[3];
        if alpha == 0.0 {
            return;
        }

        match border_style {
            BorderStyle::None | BorderStyle::Hidden => return,
            BorderStyle::Dotted | BorderStyle::Dashed => {
                // Use stroke for dotted/dashed borders
                let (dash_length, gap_length, cap) = if matches!(border_style, BorderStyle::Dotted)
                {
                    // Dotted: use small circular dots (width determines size)
                    // Per CSS spec, dots are circular (round caps) with equal dash and gap
                    let dot_size = border_width.max(1.0);
                    (dot_size, dot_size, Cap::Round)
                } else {
                    // Dashed: use longer dashes (typically 3x width per CSS spec)
                    // Dashes use square/butt caps
                    let dash_size = border_width * 3.0;
                    (dash_size, dash_size, Cap::Butt)
                };

                let stroke = Stroke {
                    width: border_width,
                    dash_pattern: vec![dash_length, gap_length].into(),
                    dash_offset: 0.0,
                    start_cap: cap,
                    end_cap: cap,
                    ..Stroke::default()
                };

                sb.stroke(&stroke, self.transform, color, None, &path);
            }
            BorderStyle::Solid
            | BorderStyle::Double
            | BorderStyle::Groove
            | BorderStyle::Ridge
            | BorderStyle::Inset
            | BorderStyle::Outset => {
                // Use fill for solid borders (and fallback for unimplemented styles)
                sb.fill(Fill::NonZero, self.transform, color, None, &path);
            }
        }
    }

    /// ✅ dotted - Defines a dotted border
    /// ✅ dashed - Defines a dashed border
    /// ✅ solid - Defines a solid border
    /// ❌ double - Defines a double border
    /// ❌ groove - Defines a 3D grooved border. The effect depends on the border-color value
    /// ❌ ridge - Defines a 3D ridged border. The effect depends on the border-color value
    /// ❌ inset - Defines a 3D inset border. The effect depends on the border-color value
    /// ❌ outset - Defines a 3D outset border. The effect depends on the border-color value
    /// ✅ none - Defines no border
    /// ✅ hidden - Defines a hidden border
    fn draw_outline(&self, scene: &mut impl PaintScene) {
        let outline = self.style.get_outline();

        let current_color = self.style.clone_color();
        let color = outline
            .outline_color
            .resolve_to_absolute(&current_color)
            .as_srgb_color();

        let style = match outline.outline_style {
            OutlineStyle::Auto => return,
            OutlineStyle::BorderStyle(style) => style,
        };

        let outline_width = self.frame.outline_width;

        match style {
            BorderStyle::None | BorderStyle::Hidden => return,
            BorderStyle::Dotted | BorderStyle::Dashed => {
                // Use stroke for dotted/dashed outlines
                let path = self.frame.outline();
                let (dash_length, gap_length, cap) = if matches!(style, BorderStyle::Dotted) {
                    let dot_size = outline_width.max(1.0);
                    (dot_size, dot_size, Cap::Round)
                } else {
                    let dash_size = outline_width * 3.0;
                    (dash_size, dash_size, Cap::Butt)
                };

                let stroke = Stroke {
                    width: outline_width,
                    dash_pattern: vec![dash_length, gap_length].into(),
                    dash_offset: 0.0,
                    start_cap: cap,
                    end_cap: cap,
                    ..Stroke::default()
                };

                scene.stroke(&stroke, self.transform, color, None, &path);
            }
            BorderStyle::Solid => {
                let path = self.frame.outline();
                scene.fill(Fill::NonZero, self.transform, color, None, &path);
            }
            // TODO: Implement other border styles
            BorderStyle::Inset
            | BorderStyle::Groove
            | BorderStyle::Outset
            | BorderStyle::Ridge
            | BorderStyle::Double => {
                let path = self.frame.outline();
                scene.fill(Fill::NonZero, self.transform, color, None, &path);
            }
        }
    }
}
impl<'a> std::ops::Deref for ElementCx<'a> {
    type Target = BlitzDomPainter<'a>;
    fn deref(&self) -> &Self::Target {
        self.context
    }
}

fn insets_from_taffy_rect(input: taffy::Rect<f64>) -> Insets {
    Insets {
        x0: input.left,
        y0: input.top,
        x1: input.right,
        y1: input.bottom,
    }
}

/// Convert Stylo and Taffy types into Kurbo types
fn create_css_rect(style: &ComputedValues, layout: &Layout, scale: f64) -> CssBox {
    // Resolve and rescale
    // We have to scale since document pixels are not same same as rendered pixels
    let width: f64 = layout.size.width as f64;
    let height: f64 = layout.size.height as f64;
    let border_box = Rect::new(0.0, 0.0, width * scale, height * scale);
    let border = insets_from_taffy_rect(layout.border.map(|p| p as f64 * scale));
    let padding = insets_from_taffy_rect(layout.padding.map(|p| p as f64 * scale));
    let outline_width = style.get_outline().outline_width.to_f64_px() * scale;
    let margin = insets_from_taffy_rect(layout.margin.map(|p| p as f64 * scale));

    // Resolve the radii to a length. need to downscale since the radii are in document pixels
    let resolve_w = CSSPixelLength::new(width as _);
    let resolve_h = CSSPixelLength::new(height as _);
    let resolve_radii = |radius: &BorderCornerRadius| -> Vec2 {
        Vec2 {
            x: scale * radius.0.width.0.resolve(resolve_w).px() as f64,
            y: scale * radius.0.height.0.resolve(resolve_h).px() as f64,
        }
    };
    let s_border = style.get_border();
    let border_radii = NonUniformRoundedRectRadii {
        top_left: resolve_radii(&s_border.border_top_left_radius),
        top_right: resolve_radii(&s_border.border_top_right_radius),
        bottom_right: resolve_radii(&s_border.border_bottom_right_radius),
        bottom_left: resolve_radii(&s_border.border_bottom_left_radius),
    };

    CssBox::new(
        border_box,
        border,
        padding,
        margin,
        outline_width,
        border_radii,
    )
}

fn resolve_length_percentage_value(value: &LengthPercentage, axis: f64) -> f64 {
    resolve_unpacked_value(value.unpack(), axis)
}

fn resolve_non_negative_length_percentage_value(
    value: &NonNegativeLengthPercentage,
    axis: f64,
) -> f64 {
    resolve_unpacked_value(value.0.unpack(), axis)
}

fn resolve_unpacked_value(unpacked: Unpacked, axis: f64) -> f64 {
    match unpacked {
        Unpacked::Length(len) => len.px() as f64,
        Unpacked::Percentage(pct) => axis * pct.to_percentage() as f64,
        Unpacked::Calc(calc) => calc.resolve(CSSPixelLength::new(axis as f32)).px() as f64,
    }
}

fn resolve_shape_position(
    position: GenericPositionOrAuto<Position>,
    reference_rect: Rect,
) -> Point {
    match position {
        GenericPositionOrAuto::Position(pos) => {
            let x = resolve_unpacked_value(pos.horizontal.unpack(), reference_rect.width());
            let y = resolve_unpacked_value(pos.vertical.unpack(), reference_rect.height());
            Point {
                x: reference_rect.origin().x + x,
                y: reference_rect.origin().y + y,
            }
        }
        GenericPositionOrAuto::Auto => Point {
            x: reference_rect.origin().x + reference_rect.width() / 2.0,
            y: reference_rect.origin().y + reference_rect.height() / 2.0,
        },
    }
}

fn resolve_shape_radius(
    radius: &ShapeRadius<NonNegativeLengthPercentage>,
    center: Point,
    reference_rect: Rect,
    axis: f64,
) -> f64 {
    match radius {
        ShapeRadius::Length(length) => resolve_non_negative_length_percentage_value(length, axis),
        ShapeRadius::ClosestSide => {
            let distances = [
                center.x - reference_rect.x0,
                reference_rect.x1 - center.x,
                center.y - reference_rect.y0,
                reference_rect.y1 - center.y,
            ];
            distances
                .iter()
                .fold(f64::INFINITY, |acc, value| acc.min(*value))
        }
        ShapeRadius::FarthestSide => {
            let distances = [
                center.x - reference_rect.x0,
                reference_rect.x1 - center.x,
                center.y - reference_rect.y0,
                reference_rect.y1 - center.y,
            ];
            distances.iter().fold(0.0, |acc, value| acc.max(*value))
        }
    }
}

/// CSS default value for shape-margin property (0 per CSS spec).
const DEFAULT_SHAPE_MARGIN: f64 = 0.0;

/// Expands a polygon point outward by the given margin.
///
/// The expansion is done by scaling the point away from the bounding box center.
/// This is a simple approximation; a more accurate implementation would expand
/// perpendicular to the polygon edge, but this approach works well for most cases.
fn expand_polygon_point(point: Point, bounding_box_center: Point, margin: f64) -> Point {
    if margin <= 0.0 {
        return point;
    }

    let dx = point.x - bounding_box_center.x;
    let dy = point.y - bounding_box_center.y;
    let dist = (dx * dx + dy * dy).sqrt();

    if dist > 0.0 {
        let scale = (dist + margin) / dist;
        Point {
            x: bounding_box_center.x + dx * scale,
            y: bounding_box_center.y + dy * scale,
        }
    } else {
        point
    }
}

/// Resolves the shape-margin value.
///
/// Stylo 0.9.0 doesn’t expose `shape-margin` in `ComputedValues` for Servo, so we
/// implement a lightweight parser that understands inline style declarations:
/// `style="shape-margin: 12px"` or percentages (resolved against the reference
/// rect width). Unknown or negative values fall back to the CSS default of 0.
fn resolve_shape_margin_for_node(node: &Node, reference_rect: Rect) -> f64 {
    let Some(style_attr) = node.attr(local_name!("style")) else {
        return DEFAULT_SHAPE_MARGIN;
    };

    let margin_value = style_attr.split(';').find_map(|decl| {
        let mut parts = decl.splitn(2, ':');
        let name = parts.next()?.trim();
        let value = parts.next()?.trim();
        if name.eq_ignore_ascii_case("shape-margin") {
            Some(value)
        } else {
            None
        }
    });

    let Some(raw_value) = margin_value else {
        return DEFAULT_SHAPE_MARGIN;
    };

    parse_shape_margin_value(raw_value, reference_rect).unwrap_or(DEFAULT_SHAPE_MARGIN)
}

fn parse_shape_margin_value(raw_value: &str, reference_rect: Rect) -> Option<f64> {
    let mut input = ParserInput::new(raw_value);
    let mut parser = Parser::new(&mut input);

    let px_value = match parser.next() {
        Ok(Token::Percentage { unit_value, .. }) => reference_rect.width() * *unit_value as f64,
        Ok(Token::Dimension { value, unit, .. }) if unit.eq_ignore_ascii_case("px") => {
            *value as f64
        }
        Ok(Token::Number { value, .. }) => *value as f64, // treat unitless as px for convenience
        _ => return None,
    };

    Some(px_value.max(0.0))
}

fn convert_shape_commands_to_absolute(
    commands: &[GenericShapeCommand<Angle, LengthPercentage>],
    box_width: f64,
    box_height: f64,
) -> Vec<GenericShapeCommand<f32, f32>> {
    commands
        .iter()
        .map(|cmd| match cmd {
            GenericShapeCommand::Move { point } => GenericShapeCommand::Move {
                point: convert_command_endpoint(point, box_width, box_height),
            },
            GenericShapeCommand::Line { point } => GenericShapeCommand::Line {
                point: convert_command_endpoint(point, box_width, box_height),
            },
            GenericShapeCommand::HLine { by_to, x } => GenericShapeCommand::HLine {
                by_to: *by_to,
                x: resolve_length_percentage_value(x, box_width) as f32,
            },
            GenericShapeCommand::VLine { by_to, y } => GenericShapeCommand::VLine {
                by_to: *by_to,
                y: resolve_length_percentage_value(y, box_height) as f32,
            },
            GenericShapeCommand::CubicCurve {
                point,
                control1,
                control2,
            } => GenericShapeCommand::CubicCurve {
                point: convert_command_endpoint(point, box_width, box_height),
                control1: coordinate_pair_to_f32(control1, box_width, box_height),
                control2: coordinate_pair_to_f32(control2, box_width, box_height),
            },
            GenericShapeCommand::QuadCurve { point, control1 } => GenericShapeCommand::QuadCurve {
                point: convert_command_endpoint(point, box_width, box_height),
                control1: coordinate_pair_to_f32(control1, box_width, box_height),
            },
            GenericShapeCommand::SmoothCubic { point, control2 } => {
                GenericShapeCommand::SmoothCubic {
                    point: convert_command_endpoint(point, box_width, box_height),
                    control2: coordinate_pair_to_f32(control2, box_width, box_height),
                }
            }
            GenericShapeCommand::SmoothQuad { point } => GenericShapeCommand::SmoothQuad {
                point: convert_command_endpoint(point, box_width, box_height),
            },
            GenericShapeCommand::Arc {
                point,
                radii,
                arc_sweep,
                arc_size,
                rotate,
            } => GenericShapeCommand::Arc {
                point: convert_command_endpoint(point, box_width, box_height),
                radii: coordinate_pair_to_f32(radii, box_width, box_height),
                arc_sweep: *arc_sweep,
                arc_size: *arc_size,
                rotate: rotate.degrees(),
            },
            GenericShapeCommand::Close => GenericShapeCommand::Close,
        })
        .collect()
}

fn convert_command_endpoint(
    endpoint: &CommandEndPoint<LengthPercentage>,
    box_width: f64,
    box_height: f64,
) -> CommandEndPoint<f32> {
    match endpoint {
        CommandEndPoint::ToPosition(pos) => CommandEndPoint::ToPosition(GenericPosition {
            horizontal: resolve_length_percentage_value(&pos.horizontal, box_width) as f32,
            vertical: resolve_length_percentage_value(&pos.vertical, box_height) as f32,
        }),
        CommandEndPoint::ByCoordinate(coord) => {
            CommandEndPoint::ByCoordinate(coordinate_pair_to_f32(coord, box_width, box_height))
        }
    }
}

fn coordinate_pair_to_f32(
    pair: &CoordinatePair<LengthPercentage>,
    box_width: f64,
    box_height: f64,
) -> CoordinatePair<f32> {
    CoordinatePair {
        x: resolve_length_percentage_value(&pair.x, box_width) as f32,
        y: resolve_length_percentage_value(&pair.y, box_height) as f32,
    }
}

fn coordinate_pair_to_point(pair: &CoordinatePair<f32>) -> Point {
    Point {
        x: pair.x as f64,
        y: pair.y as f64,
    }
}

fn resolve_endpoint_absolute(point: &CommandEndPoint<f32>, current: Point) -> Point {
    match point {
        CommandEndPoint::ToPosition(position) => Point {
            x: position.horizontal as f64,
            y: position.vertical as f64,
        },
        CommandEndPoint::ByCoordinate(coord) => Point {
            x: current.x + coord.x as f64,
            y: current.y + coord.y as f64,
        },
    }
}

fn ensure_path_started(path: &mut BezPath, has_started: &mut bool, current: Point) {
    if !*has_started {
        path.move_to(current);
        *has_started = true;
    }
}

fn reflect_point(control: Point, center: Point) -> Point {
    Point {
        x: 2.0 * center.x - control.x,
        y: 2.0 * center.y - control.y,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use style::values::computed::Percentage;
    use style::values::computed::length_percentage::Unpacked;
    use style::values::generics::basic_shape::ShapeRadius;

    #[test]
    fn test_parse_shape_margin_returns_none_for_empty() {
        let reference_rect = Rect::new(0.0, 0.0, 100.0, 100.0);
        let margin = parse_shape_margin_value("", reference_rect);
        assert!(margin.is_none());
    }

    #[test]
    fn test_resolve_unpacked_value_length() {
        let unpacked = Unpacked::Length(CSSPixelLength::new(42.0));
        let axis = 100.0;

        let result = resolve_unpacked_value(unpacked, axis);

        assert_eq!(result, 42.0, "Length value should be resolved correctly");
    }

    #[test]
    fn test_resolve_unpacked_value_percentage() {
        let unpacked = Unpacked::Percentage(Percentage(0.5));
        let axis = 200.0;

        let result = resolve_unpacked_value(unpacked, axis);

        assert_eq!(
            result, 100.0,
            "Percentage value should be resolved correctly (50% of 200 = 100)"
        );
    }

    #[test]
    fn test_resolve_shape_radius_closest_side() {
        let radius = ShapeRadius::ClosestSide;
        // Center at (25, 25) in a 100x100 rect
        // Closest side is left (25) or top (25)
        let center = Point::new(25.0, 25.0);
        let reference_rect = Rect::new(0.0, 0.0, 100.0, 100.0);
        let axis = 100.0;

        let result = resolve_shape_radius(&radius, center, reference_rect, axis);

        assert_eq!(
            result, 25.0,
            "ClosestSide should return minimum distance to any edge"
        );
    }

    #[test]
    fn test_resolve_shape_radius_farthest_side() {
        let radius = ShapeRadius::FarthestSide;
        // Center at (25, 25) in a 100x100 rect
        // Farthest side is right (75) or bottom (75)
        let center = Point::new(25.0, 25.0);
        let reference_rect = Rect::new(0.0, 0.0, 100.0, 100.0);
        let axis = 100.0;

        let result = resolve_shape_radius(&radius, center, reference_rect, axis);

        assert_eq!(
            result, 75.0,
            "FarthestSide should return maximum distance to any edge"
        );
    }

    #[test]
    fn test_resolve_shape_position_auto() {
        let position = GenericPositionOrAuto::Auto;
        let reference_rect = Rect::new(10.0, 20.0, 110.0, 120.0);

        let result = resolve_shape_position(position, reference_rect);

        // Auto should center at (10 + 50, 20 + 50) = (60, 70)
        let expected = Point::new(60.0, 70.0);
        assert_eq!(
            result.x, expected.x,
            "Auto position should center horizontally"
        );
        assert_eq!(
            result.y, expected.y,
            "Auto position should center vertically"
        );
    }

    #[test]
    fn test_parse_shape_margin_px() {
        let reference_rect = Rect::new(0.0, 0.0, 200.0, 100.0);
        let parsed = parse_shape_margin_value("15px", reference_rect).unwrap();
        assert_eq!(parsed, 15.0);
    }

    #[test]
    fn test_parse_shape_margin_percentage() {
        let reference_rect = Rect::new(0.0, 0.0, 200.0, 100.0);
        let parsed = parse_shape_margin_value("10%", reference_rect).unwrap();
        assert_eq!(parsed, 20.0);
    }

    #[test]
    fn test_parse_shape_margin_invalid() {
        let reference_rect = Rect::new(0.0, 0.0, 200.0, 100.0);
        let parsed = parse_shape_margin_value("bogus", reference_rect);
        assert!(parsed.is_none());
    }

    #[test]
    fn test_resolve_shape_radius_edge_cases() {
        // Test edge cases for shape radius resolution
        let reference_rect = Rect::new(0.0, 0.0, 100.0, 100.0);
        let axis = 100.0;

        // Center at exact center
        let center_center = Point::new(50.0, 50.0);
        let radius_closest = ShapeRadius::ClosestSide;
        let result = resolve_shape_radius(&radius_closest, center_center, reference_rect, axis);
        assert_eq!(
            result, 50.0,
            "Center point should have equal distance to all sides"
        );

        // Center at corner
        let center_corner = Point::new(0.0, 0.0);
        let result = resolve_shape_radius(&radius_closest, center_corner, reference_rect, axis);
        assert_eq!(
            result, 0.0,
            "Corner point should have 0 distance to closest side"
        );

        // Center at edge
        let center_edge = Point::new(0.0, 50.0);
        let result = resolve_shape_radius(&radius_closest, center_edge, reference_rect, axis);
        assert_eq!(
            result, 0.0,
            "Edge point should have 0 distance to closest side"
        );
    }

    #[test]
    fn test_resolve_unpacked_value_zero_axis() {
        // Test edge case: zero axis
        let unpacked = Unpacked::Percentage(Percentage(0.5));
        let axis = 0.0;

        let result = resolve_unpacked_value(unpacked, axis);

        assert_eq!(result, 0.0, "Percentage of zero axis should be zero");
    }

    #[test]
    fn test_resolve_shape_radius_center_at_origin() {
        // Test when center is at origin
        let radius = ShapeRadius::ClosestSide;
        let center = Point::new(0.0, 0.0);
        let reference_rect = Rect::new(0.0, 0.0, 100.0, 100.0);
        let axis = 100.0;

        let result = resolve_shape_radius(&radius, center, reference_rect, axis);

        assert_eq!(
            result, 0.0,
            "Center at origin should have 0 distance to closest side"
        );
    }

    #[test]
    fn test_expand_polygon_point_no_margin() {
        // Test that zero margin returns the point unchanged
        let point = Point::new(10.0, 20.0);
        let center = Point::new(50.0, 50.0);
        let expanded = expand_polygon_point(point, center, 0.0);
        assert_eq!(expanded.x, point.x);
        assert_eq!(expanded.y, point.y);
    }

    #[test]
    fn test_expand_polygon_point_with_margin() {
        // Test that margin expands point away from center
        let point = Point::new(60.0, 50.0); // 10 units right of center
        let center = Point::new(50.0, 50.0);
        let margin = 5.0;
        let expanded = expand_polygon_point(point, center, margin);

        // Should be 15 units right of center (10 + 5)
        assert_eq!(expanded.x, 65.0);
        assert_eq!(expanded.y, 50.0);
    }

    #[test]
    fn test_expand_polygon_point_at_center() {
        // Test that point at center remains unchanged (dist = 0)
        let point = Point::new(50.0, 50.0);
        let center = Point::new(50.0, 50.0);
        let margin = 10.0;
        let expanded = expand_polygon_point(point, center, margin);

        // Point at center should remain unchanged
        assert_eq!(expanded.x, point.x);
        assert_eq!(expanded.y, point.y);
    }

    #[test]
    fn test_expand_polygon_point_diagonal() {
        // Test expansion in diagonal direction
        let point = Point::new(60.0, 60.0); // sqrt(2)*10 units from (50, 50)
        let center = Point::new(50.0, 50.0);
        let margin = 5.0;
        let expanded = expand_polygon_point(point, center, margin);

        // Should maintain direction but increase distance
        let original_dist = ((point.x - center.x).powi(2) + (point.y - center.y).powi(2)).sqrt();
        let expanded_dist =
            ((expanded.x - center.x).powi(2) + (expanded.y - center.y).powi(2)).sqrt();
        assert!(
            (expanded_dist - (original_dist + margin)).abs() < 1e-10,
            "Distance should increase by margin"
        );

        // Direction should be preserved (ratio should be same)
        let original_ratio = (point.y - center.y) / (point.x - center.x);
        let expanded_ratio = (expanded.y - center.y) / (expanded.x - center.x);
        assert!(
            (original_ratio - expanded_ratio).abs() < 1e-10,
            "Direction should be preserved"
        );
    }
}

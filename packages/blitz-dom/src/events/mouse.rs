use blitz_traits::{
    events::{
        BlitzInputEvent, BlitzMouseButtonEvent, DomEvent, DomEventData, MouseEventButton,
        MouseEventButtons,
    },
    navigation::NavigationOptions,
};
use markup5ever::local_name;

use crate::{BaseDocument, node::SpecialElementData};

pub(crate) fn handle_mousemove(
    doc: &mut BaseDocument,
    target: usize,
    x: f32,
    y: f32,
    buttons: MouseEventButtons,
) -> bool {
    let mut changed = doc.set_hover_to(x, y);

    // If buttons are pressed and there's a focused input field, continue extending selection
    // even when mouse moves outside the input bounds
    if buttons != MouseEventButtons::None {
        if let Some(focused_node_id) = doc.focus_node_id {
            // Get node layout info before borrowing mutably
            let (content_box_offset, node_x, node_y, node_width, node_height) = {
                let node = &doc.nodes[focused_node_id];
                (
                    taffy::Point {
                        x: node.final_layout.padding.left + node.final_layout.border.left,
                        y: node.final_layout.padding.top + node.final_layout.border.top,
                    },
                    node.final_layout.content_box_x(),
                    node.final_layout.content_box_y(),
                    node.final_layout.content_box_width(),
                    node.final_layout.content_box_height(),
                )
            };

            // First, do hit test to check if we're over the input
            let hit_result = doc.hit(x, y);

            // Calculate coordinates
            let (rel_x, rel_y) = if let Some(hit) = hit_result {
                if hit.node_id == focused_node_id {
                    // Mouse is over the input, use hit coordinates
                    (hit.x, hit.y)
                } else {
                    // Mouse is outside, clamp to input bounds
                    let abs_x = x / doc.viewport.scale_f64() as f32;
                    let abs_y = y / doc.viewport.scale_f64() as f32;
                    
                    let clamped_x = (abs_x - node_x - content_box_offset.x)
                        .max(0.0)
                        .min(node_width);
                    let clamped_y = (abs_y - node_y - content_box_offset.y)
                        .max(0.0)
                        .min(node_height);
                    
                    (clamped_x, clamped_y)
                }
            } else {
                // Mouse is outside, clamp to input bounds
                let abs_x = x / doc.viewport.scale_f64() as f32;
                let abs_y = y / doc.viewport.scale_f64() as f32;
                
                let clamped_x = (abs_x - node_x - content_box_offset.x)
                    .max(0.0)
                    .min(node_width);
                let clamped_y = (abs_y - node_y - content_box_offset.y)
                    .max(0.0)
                    .min(node_height);
                
                (clamped_x, clamped_y)
            };

            // Now borrow mutably to extend selection
            let node = &mut doc.nodes[focused_node_id];
            if let Some(el) = node.data.downcast_element_mut() {
                if let SpecialElementData::TextInput(ref mut text_input_data) = el.special_data {
                    let scaled_x = (rel_x - content_box_offset.x) as f64 * doc.viewport.scale_f64();
                    let scaled_y = (rel_y - content_box_offset.y) as f64 * doc.viewport.scale_f64();

                    text_input_data
                        .editor
                        .driver(&mut doc.font_ctx.lock().unwrap(), &mut doc.layout_ctx)
                        .extend_selection_to_point(scaled_x as f32, scaled_y as f32);

                    changed = true;
                    return changed;
                }
            }
        }
    }

    // Original behavior: only extend selection when mouse is over the input
    let Some(hit) = doc.hit(x, y) else {
        return changed;
    };

    if hit.node_id != target {
        return changed;
    }

    let node = &mut doc.nodes[target];
    let Some(el) = node.data.downcast_element_mut() else {
        return changed;
    };

    let disabled = el.attr(local_name!("disabled")).is_some();
    if disabled {
        return changed;
    }

    if let SpecialElementData::TextInput(ref mut text_input_data) = el.special_data {
        if buttons == MouseEventButtons::None {
            return changed;
        }

        let content_box_offset = taffy::Point {
            x: node.final_layout.padding.left + node.final_layout.border.left,
            y: node.final_layout.padding.top + node.final_layout.border.top,
        };

        let x = (hit.x - content_box_offset.x) as f64 * doc.viewport.scale_f64();
        let y = (hit.y - content_box_offset.y) as f64 * doc.viewport.scale_f64();

        text_input_data
            .editor
            .driver(&mut doc.font_ctx.lock().unwrap(), &mut doc.layout_ctx)
            .extend_selection_to_point(x as f32, y as f32);

        changed = true;
    }

    changed
}

pub(crate) fn handle_mousedown(doc: &mut BaseDocument, target: usize, x: f32, y: f32) {
    let Some(hit) = doc.hit(x, y) else {
        return;
    };
    if hit.node_id != target {
        return;
    }

    let node = &mut doc.nodes[target];
    let Some(el) = node.data.downcast_element_mut() else {
        return;
    };

    let disabled = el.attr(local_name!("disabled")).is_some();
    if disabled {
        return;
    }

    if let SpecialElementData::TextInput(ref mut text_input_data) = el.special_data {
        let content_box_offset = taffy::Point {
            x: node.final_layout.padding.left + node.final_layout.border.left,
            y: node.final_layout.padding.top + node.final_layout.border.top,
        };
        let x = (hit.x - content_box_offset.x) as f64 * doc.viewport.scale_f64();
        let y = (hit.y - content_box_offset.y) as f64 * doc.viewport.scale_f64();

        // On mousedown, collapse selection first, then move cursor
        // This ensures we place the cursor instead of starting a selection
        {
            let mut font_ctx = doc.font_ctx.lock().unwrap();
            let mut driver = text_input_data.editor.driver(&mut font_ctx, &mut doc.layout_ctx);
            driver.collapse_selection();
            driver.move_to_point(x as f32, y as f32);
        }

        doc.set_focus_to(hit.node_id);
    }
}

// Helper function to check if a node is a text input or is inside a text input
fn is_text_input_or_inside(doc: &BaseDocument, node_id: usize) -> bool {
    let mut current_id = Some(node_id);
    while let Some(id) = current_id {
        let node = &doc.nodes[id];
        if let Some(el) = node.data.downcast_element() {
            if matches!(el.special_data, SpecialElementData::TextInput(_)) {
                return true;
            }
        }
        current_id = node.parent;
    }
    false
}

pub(crate) fn handle_mouseup<F: FnMut(DomEvent)>(
    doc: &mut BaseDocument,
    target: usize,
    event: &BlitzMouseButtonEvent,
    mut dispatch_event: F,
) {
    // If there's a focused input field and we're releasing the mouse button,
    // finalize the selection only if mousedown was inside the input (text selection operation)
    if event.button == MouseEventButton::Main {
        if let Some(focused_node_id) = doc.focus_node_id {
            // Only finalize selection if mousedown was inside the input
            // This distinguishes text selection (mousedown inside) from clicking outside (mousedown outside)
            let mousedown_was_inside = doc.mousedown_node_id
                .map(|id| is_text_input_or_inside(doc, id))
                .unwrap_or(false);
            
            if mousedown_was_inside {
                // First, do hit test before borrowing mutably
                let hit_result = doc.hit(event.x, event.y);
                
                // Get layout info before borrowing mutably
                let (content_box_offset, node_x, node_y, node_width, node_height) = {
                    let node = &doc.nodes[focused_node_id];
                    (
                        taffy::Point {
                            x: node.final_layout.padding.left + node.final_layout.border.left,
                            y: node.final_layout.padding.top + node.final_layout.border.top,
                        },
                        node.final_layout.content_box_x(),
                        node.final_layout.content_box_y(),
                        node.final_layout.content_box_width(),
                        node.final_layout.content_box_height(),
                    )
                };
                
                // Now borrow mutably to finalize selection
                let node = &mut doc.nodes[focused_node_id];
                if let Some(el) = node.data.downcast_element_mut() {
                    if let SpecialElementData::TextInput(ref mut text_input_data) = el.special_data {
                        // Calculate final selection point
                        let (scaled_x, scaled_y) = if let Some(hit) = hit_result {
                            if hit.node_id == focused_node_id {
                                // Mouse is over the input, use hit coordinates
                                let x = (hit.x - content_box_offset.x) as f64 * doc.viewport.scale_f64();
                                let y = (hit.y - content_box_offset.y) as f64 * doc.viewport.scale_f64();
                                (x, y)
                            } else {
                                // Mouse is outside input, clamp to input bounds
                                let abs_x = event.x / doc.viewport.scale_f64() as f32;
                                let abs_y = event.y / doc.viewport.scale_f64() as f32;
                                
                                let clamped_x = if abs_x < node_x {
                                    0.0
                                } else if abs_x > node_x + node_width {
                                    node_width
                                } else {
                                    (abs_x - node_x - content_box_offset.x).max(0.0).min(node_width)
                                };
                                
                                let clamped_y = if abs_y < node_y {
                                    0.0
                                } else if abs_y > node_y + node_height {
                                    node_height
                                } else {
                                    (abs_y - node_y - content_box_offset.y).max(0.0).min(node_height)
                                };
                                
                                let x = (clamped_x - content_box_offset.x) as f64 * doc.viewport.scale_f64();
                                let y = (clamped_y - content_box_offset.y) as f64 * doc.viewport.scale_f64();
                                (x, y)
                            }
                        } else {
                            // Mouse is outside any element, clamp to input bounds based on direction
                            let abs_x = event.x / doc.viewport.scale_f64() as f32;
                            let abs_y = event.y / doc.viewport.scale_f64() as f32;
                            
                            let clamped_x = if abs_x < node_x {
                                0.0
                            } else if abs_x > node_x + node_width {
                                node_width
                            } else {
                                (abs_x - node_x - content_box_offset.x).max(0.0).min(node_width)
                            };
                            
                            let clamped_y = if abs_y < node_y {
                                0.0
                            } else if abs_y > node_y + node_height {
                                node_height
                            } else {
                                (abs_y - node_y - content_box_offset.y).max(0.0).min(node_height)
                            };
                            
                            let x = (clamped_x - content_box_offset.x) as f64 * doc.viewport.scale_f64();
                            let y = (clamped_y - content_box_offset.y) as f64 * doc.viewport.scale_f64();
                            (x, y)
                        };
                        
                        // Finalize selection
                        {
                            let mut font_ctx = doc.font_ctx.lock().unwrap();
                            let mut driver = text_input_data.editor.driver(&mut font_ctx, &mut doc.layout_ctx);
                            driver.extend_selection_to_point(scaled_x as f32, scaled_y as f32);
                        }
                        
                        doc.shell_provider.request_redraw();
                    }
                }
            }
        }
    }

    if doc.devtools().highlight_hover {
        let mut node = doc.get_node(target).unwrap();
        if event.button == MouseEventButton::Secondary {
            if let Some(parent_id) = node.layout_parent.get() {
                node = doc.get_node(parent_id).unwrap();
            }
        }
        doc.debug_log_node(node.id);
        doc.devtools_mut().highlight_hover = false;
        return;
    }

    // Determine whether to dispatch a click event
    let do_click = true;
    // let do_click = doc.mouse_down_node.is_some_and(|mouse_down_id| {
    //     // Anonymous node ids are unstable due to tree reconstruction. So we compare the id
    //     // of the first non-anonymous ancestor.
    //     mouse_down_id == target
    //         || doc.non_anon_ancestor_if_anon(mouse_down_id) == doc.non_anon_ancestor_if_anon(target)
    // });

    // Dispatch a click event
    if do_click && event.button == MouseEventButton::Main {
        dispatch_event(DomEvent::new(target, DomEventData::Click(event.clone())));
    }
}

pub(crate) fn handle_click<F: FnMut(DomEvent)>(
    doc: &mut BaseDocument,
    target: usize,
    event: &BlitzMouseButtonEvent,
    mut dispatch_event: F,
) {
    let mut maybe_node_id = Some(target);
    while let Some(node_id) = maybe_node_id {
        let maybe_element = {
            let node = &mut doc.nodes[node_id];
            node.data.downcast_element_mut()
        };

        let Some(el) = maybe_element else {
            maybe_node_id = doc.nodes[node_id].parent;
            continue;
        };

        let disabled = el.attr(local_name!("disabled")).is_some();
        if disabled {
            return;
        }

        if let SpecialElementData::TextInput(_) = el.special_data {
            return;
        }

        match el.name.local {
            local_name!("input") if el.attr(local_name!("type")) == Some("checkbox") => {
                let is_checked = BaseDocument::toggle_checkbox(el);
                let value = is_checked.to_string();
                dispatch_event(DomEvent::new(
                    node_id,
                    DomEventData::Input(BlitzInputEvent { value }),
                ));
                doc.set_focus_to(node_id);
                return;
            }
            local_name!("input") if el.attr(local_name!("type")) == Some("radio") => {
                let radio_set = el.attr(local_name!("name")).unwrap().to_string();
                BaseDocument::toggle_radio(doc, radio_set, node_id);

                // TODO: make input event conditional on value actually changing
                let value = String::from("true");
                dispatch_event(DomEvent::new(
                    node_id,
                    DomEventData::Input(BlitzInputEvent { value }),
                ));

                BaseDocument::set_focus_to(doc, node_id);

                return;
            }
            // Clicking labels triggers click, and possibly input event, of associated input
            local_name!("label") => {
                if let Some(target_node_id) = doc.label_bound_input_element(node_id).map(|n| n.id) {
                    // Apply default click event action for target node
                    let target_node = doc.get_node_mut(target_node_id).unwrap();
                    let syn_event = target_node.synthetic_click_event_data(event.mods);
                    handle_click(doc, target_node_id, &syn_event, dispatch_event);
                    return;
                }
            }
            local_name!("a") => {
                if let Some(href) = el.attr(local_name!("href")) {
                    if let Some(url) = doc.url.resolve_relative(href) {
                        doc.navigation_provider.navigate_to(NavigationOptions::new(
                            url,
                            String::from("text/plain"),
                            doc.id(),
                        ));
                    } else {
                        println!("{href} is not parseable as a url. : {:?}", *doc.url)
                    }
                    return;
                } else {
                    println!("Clicked link without href: {:?}", el.attrs());
                }
            }
            local_name!("input")
                if el.is_submit_button() || el.attr(local_name!("type")) == Some("submit") =>
            {
                if let Some(form_owner) = doc.controls_to_form.get(&node_id) {
                    doc.submit_form(*form_owner, node_id);
                }
            }
            #[cfg(feature = "file_input")]
            local_name!("input") if el.attr(local_name!("type")) == Some("file") => {
                use crate::qual_name;
                //TODO: Handle accept attribute https://developer.mozilla.org/en-US/docs/Web/HTML/Reference/Attributes/accept by passing an appropriate filter
                let multiple = el.attr(local_name!("multiple")).is_some();
                let files = doc.shell_provider.open_file_dialog(multiple, None);

                if let Some(file) = files.first() {
                    el.attrs
                        .set(qual_name!("value", html), &file.to_string_lossy());
                }
                let text_content = match files.len() {
                    0 => "No Files Selected".to_string(),
                    1 => files
                        .first()
                        .unwrap()
                        .file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .to_string(),
                    x => format!("{x} Files Selected"),
                };

                if files.is_empty() {
                    el.special_data = SpecialElementData::None;
                } else {
                    el.special_data = SpecialElementData::FileInput(files.into())
                }
                let child_label_id = doc.nodes[node_id].children[1];
                let child_text_id = doc.nodes[child_label_id].children[0];
                let text_data = doc.nodes[child_text_id]
                    .text_data_mut()
                    .expect("Text data not found");
                text_data.content = text_content;
            }
            _ => {}
        }

        // No match. Recurse up to parent.
        maybe_node_id = doc.nodes[node_id].parent;
    }

    // If nothing is matched, handle text input focus/selection
    // Only clear selection/focus if BOTH mousedown AND click target are outside the focused input
    // This prevents clearing selection after text selection (mousedown inside, mouseup outside)
    if let Some(focused_id) = doc.focus_node_id {
        // Check if the focused node is a text input
        let is_focused_text_input = {
            let node = &doc.nodes[focused_id];
            node.data.downcast_element()
                .map(|el| matches!(el.special_data, SpecialElementData::TextInput(_)))
                .unwrap_or(false)
        };
        
        if is_focused_text_input {
            // Check if click target is outside the input
            let click_target_is_outside = !is_text_input_or_inside(doc, target);
            
            // Check if mousedown was inside the input
            let mousedown_was_inside = doc.mousedown_node_id
                .map(|id| is_text_input_or_inside(doc, id))
                .unwrap_or(false);
            
            // Clear focus if:
            // 1. Click target is outside AND mousedown was outside (full click outside)
            // 2. Click target is outside AND mousedown_node_id is None (can't determine, but click is outside)
            // This preserves focus during text selection (mousedown inside, mouseup outside)
            if click_target_is_outside && !mousedown_was_inside {
                // Clear selection first
                {
                    let node = &mut doc.nodes[focused_id];
                    if let Some(el) = node.data.downcast_element_mut() {
                        if let SpecialElementData::TextInput(ref mut text_input_data) = el.special_data {
                            let mut font_ctx = doc.font_ctx.lock().unwrap();
                            let mut driver = text_input_data.editor.driver(&mut font_ctx, &mut doc.layout_ctx);
                            driver.collapse_selection();
                        }
                    }
                }
                // Clear focus (which will remove the cursor)
                doc.clear_focus();
                return;
            } else if mousedown_was_inside {
                // Mousedown was inside (text selection) - preserve focus and selection
                return;
            }
            // If click target is inside, preserve focus (fall through to return at end)
            return;
        }
    }
    
    // Clear focus for non-text-input elements or when no text input is focused
    doc.clear_focus();
}

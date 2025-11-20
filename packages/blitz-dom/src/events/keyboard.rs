use crate::{
    BaseDocument,
    node::{TextBrush, TextInputData},
};
use blitz_traits::{
    events::{BlitzInputEvent, BlitzKeyEvent, DomEvent, DomEventData},
    shell::ShellProvider,
};
use keyboard_types::{Code, Key, Modifiers};
use markup5ever::local_name;
use parley::{FontContext, LayoutContext};

// TODO: support keypress events
enum GeneratedEvent {
    Input,
    Submit,
}

pub(crate) fn handle_keypress<F: FnMut(DomEvent)>(
    doc: &mut BaseDocument,
    target: usize,
    event: BlitzKeyEvent,
    mut dispatch_event: F,
) {
    if event.key == Key::Tab {
        doc.focus_next_node();
        return;
    }

    // Use the focused node if available, otherwise use the target from the event
    // This ensures keyboard shortcuts work even if the event target doesn't match exactly
    let node_id = doc.focus_node_id.unwrap_or(target);

    let node = &mut doc.nodes[node_id];
    let Some(element_data) = node.element_data_mut() else {
        return;
    };

    if let Some(input_data) = element_data.text_input_data_mut() {
        let generated_event = apply_keypress_event(
            input_data,
            &mut doc.font_ctx.lock().unwrap(),
            &mut doc.layout_ctx,
            &*doc.shell_provider,
            event,
        );

        if let Some(generated_event) = generated_event {
            match generated_event {
                GeneratedEvent::Input => {
                    let value = input_data.editor.raw_text().to_string();
                    dispatch_event(DomEvent::new(
                        node_id,
                        DomEventData::Input(BlitzInputEvent { value }),
                    ));
                }
                GeneratedEvent::Submit => {
                    // TODO: Generate submit event that can be handled by script
                    implicit_form_submission(doc, target);
                }
            }
        }
    }
}

#[cfg(target_os = "macos")]
const ACTION_MOD: Modifiers = Modifiers::SUPER;
#[cfg(not(target_os = "macos"))]
const ACTION_MOD: Modifiers = Modifiers::CONTROL;

fn apply_keypress_event(
    input_data: &mut TextInputData,
    font_ctx: &mut FontContext,
    layout_ctx: &mut LayoutContext<TextBrush>,
    shell_provider: &dyn ShellProvider,
    event: BlitzKeyEvent,
) -> Option<GeneratedEvent> {
    // Do nothing if it is a keyup event
    if !event.state.is_pressed() {
        return None;
    }

    let mods = event.modifiers;
    let shift = mods.contains(Modifiers::SHIFT);
    let action_mod = mods.contains(ACTION_MOD);

    let is_multiline = input_data.is_multiline;
    let editor = &mut input_data.editor;
    let mut driver = editor.driver(font_ctx, layout_ctx);
    
    // Handle clipboard shortcuts (Cmd+C, Cmd+X, Cmd+V)
    // Check both the key character and the code to handle different key representations
    if action_mod {
        let key_char = match &event.key {
            Key::Character(c) => Some(c.as_str()),
            _ => None,
        };
        
        // Debug: print what we're detecting
        eprintln!("[DEBUG] action_mod=true, key={:?}, code={:?}, key_char={:?}", event.key, event.code, key_char);
        
        // Also check the code for V, C, X keys
        let is_v = key_char == Some("v") || event.code == Code::KeyV;
        let is_c = key_char == Some("c") || event.code == Code::KeyC;
        let is_x = key_char == Some("x") || event.code == Code::KeyX;
        
        eprintln!("[DEBUG] is_v={}, is_c={}, is_x={}", is_v, is_c, is_x);
        
        if is_v {
            eprintln!("[DEBUG] Cmd+V detected, attempting paste");
            std::io::Write::flush(&mut std::io::stderr()).ok();
            
            match shell_provider.get_clipboard_text() {
                Ok(text) => {
                    eprintln!("[DEBUG] Pasting text: {} ({} chars)", text.chars().take(20).collect::<String>(), text.len());
                    std::io::Write::flush(&mut std::io::stderr()).ok();
                    driver.insert_or_replace_selection(&text);
                    return Some(GeneratedEvent::Input);
                }
                Err(_e) => {
                    eprintln!("[DEBUG] Clipboard access failed - error returned from get_clipboard_text()");
                    std::io::Write::flush(&mut std::io::stderr()).ok();
                    // Clipboard access failed, silently ignore
                    return None;
                }
            }
        } else if is_c {
            if let Some(text) = driver.editor.selected_text() {
                let _ = shell_provider.set_clipboard_text(text.to_owned());
            }
            return None; // Copy doesn't generate input event
        } else if is_x {
            if let Some(text) = driver.editor.selected_text() {
                let _ = shell_provider.set_clipboard_text(text.to_owned());
                driver.delete_selection();
                return Some(GeneratedEvent::Input);
            }
            return None;
        }
    }
    
    match event.key {
        Key::Character(c) if action_mod && matches!(c.to_lowercase().as_str(), "a") => {
            if shift {
                driver.collapse_selection()
            } else {
                driver.select_all()
            }
        }
        Key::ArrowLeft => {
            if action_mod {
                if shift {
                    driver.select_word_left()
                } else {
                    driver.move_word_left()
                }
            } else if shift {
                driver.select_left()
            } else {
                driver.move_left()
            }
        }
        Key::ArrowRight => {
            if action_mod {
                if shift {
                    driver.select_word_right()
                } else {
                    driver.move_word_right()
                }
            } else if shift {
                driver.select_right()
            } else {
                driver.move_right()
            }
        }
        Key::ArrowUp => {
            if shift {
                driver.select_up()
            } else {
                driver.move_up()
            }
        }
        Key::ArrowDown => {
            if shift {
                driver.select_down()
            } else {
                driver.move_down()
            }
        }
        Key::Home => {
            if action_mod {
                if shift {
                    driver.select_to_text_start()
                } else {
                    driver.move_to_text_start()
                }
            } else if shift {
                driver.select_to_line_start()
            } else {
                driver.move_to_line_start()
            }
        }
        Key::End => {
            if action_mod {
                if shift {
                    driver.select_to_text_end()
                } else {
                    driver.move_to_text_end()
                }
            } else if shift {
                driver.select_to_line_end()
            } else {
                driver.move_to_line_end()
            }
        }
        Key::Delete => {
            if action_mod {
                driver.delete_word()
            } else {
                driver.delete()
            }
            return Some(GeneratedEvent::Input);
        }
        Key::Backspace => {
            if action_mod {
                driver.backdelete_word()
            } else {
                driver.backdelete()
            }
            return Some(GeneratedEvent::Input);
        }
        Key::Enter => {
            if is_multiline {
                driver.insert_or_replace_selection("\n");
            } else {
                return Some(GeneratedEvent::Submit);
            }
        }
        Key::Character(s) => {
            driver.insert_or_replace_selection(&s);
            return Some(GeneratedEvent::Input);
        }
        _ => {}
    };

    None
}

/// https://html.spec.whatwg.org/multipage/form-control-infrastructure.html#field-that-blocks-implicit-submission
fn implicit_form_submission(doc: &BaseDocument, text_target: usize) {
    let Some(form_owner_id) = doc.controls_to_form.get(&text_target) else {
        return;
    };
    if doc
        .controls_to_form
        .iter()
        .filter(|(_control_id, form_id)| *form_id == form_owner_id)
        .filter_map(|(control_id, _)| doc.nodes[*control_id].element_data())
        .filter(|element_data| {
            element_data.attr(local_name!("type")).is_some_and(|t| {
                matches!(
                    t,
                    "text"
                        | "search"
                        | "email"
                        | "url"
                        | "tel"
                        | "password"
                        | "date"
                        | "month"
                        | "week"
                        | "time"
                        | "datetime-local"
                        | "number"
                )
            })
        })
        .count()
        > 1
    {
        return;
    }

    doc.submit_form(*form_owner_id, *form_owner_id);
}

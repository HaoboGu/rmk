//! Unified Runnable trait implementation generator.
//!
//! Generates `Runnable` implementations for structs that combine
//! input_device, input_processor, and controller behaviors.

use proc_macro2::TokenStream;
use quote::{format_ident, quote};

use super::naming::generate_unique_variant_names;
use crate::controller::config::ControllerConfig;
use crate::input::config::{InputDeviceConfig, InputProcessorConfig};
use crate::utils::deduplicate_type_generics;

/// Build the loop body for select with optional match.
fn build_select_loop_body(
    select_arms: &[TokenStream],
    match_arms: Option<&[TokenStream]>,
) -> TokenStream {
    let select_block = quote! {
        ::rmk::futures::select_biased! {
            #(#select_arms),*
        }
    };

    match match_arms {
        Some(arms) => quote! {
            let select_result = { #select_block };
            match select_result {
                #(#arms)*
            }
        },
        None => select_block,
    }
}

/// Build the polling loop body with timer.
fn build_polling_loop_body(
    interval_ms: u64,
    timer_arm: TokenStream,
    select_arms: &[TokenStream],
    match_arms: Option<&[TokenStream]>,
) -> TokenStream {
    let select_block = quote! {
        ::rmk::futures::select_biased! {
            #timer_arm,
            #(#select_arms),*
        }
    };

    let select_handling = match match_arms {
        Some(arms) => quote! {
            let select_result = { #select_block };
            match select_result {
                #(#arms)*
            }
        },
        None => select_block,
    };

    quote! {
        let elapsed = last.elapsed();
        let interval = ::embassy_time::Duration::from_millis(#interval_ms);
        let timer = ::embassy_time::Timer::after(
            interval.checked_sub(elapsed).unwrap_or(::embassy_time::Duration::MIN)
        );
        #select_handling
    }
}

/// Generate a unified `Runnable` impl for input_device/input_processor/controller.
///
/// Handles:
/// - InputDevice: read_event + publish
/// - InputProcessor: subscribe + process
/// - Controller: subscribe + optional polling
///
/// Uses select_biased! when multiplexing multiple sources.
/// `input_device_config` and `input_processor_config` are mutually exclusive.
pub fn generate_runnable(
    struct_name: &syn::Ident,
    generics: &syn::Generics,
    where_clause: Option<&syn::WhereClause>,
    input_device_config: Option<&InputDeviceConfig>,
    input_processor_config: Option<&InputProcessorConfig>,
    controller_config: Option<&ControllerConfig>,
) -> TokenStream {
    let (impl_generics, _, _) = generics.split_for_impl();
    let ty_generics = deduplicate_type_generics(generics);

    // Helper to wrap body in Runnable impl
    let wrap_runnable = |body: TokenStream| {
        quote! {
            impl #impl_generics ::rmk::input_device::Runnable for #struct_name #ty_generics #where_clause {
                async fn run(&mut self) -> ! {
                    #body
                }
            }
        }
    };

    // Enforce mutual exclusivity.
    if input_device_config.is_some() && input_processor_config.is_some() {
        panic!("input_device and input_processor are mutually exclusive");
    }

    // Collect select arms and subscriber definitions.
    let mut sub_defs: Vec<TokenStream> = Vec::new();
    let mut select_arms: Vec<TokenStream> = Vec::new();
    let mut select_match_arms: Vec<TokenStream> = Vec::new();
    let mut use_statements: Vec<TokenStream> = Vec::new();

    let needs_split_select = input_device_config.is_some() && controller_config.is_some();
    let select_enum_name =
        needs_split_select.then(|| format_ident!("__RmkSelectEvent{}", struct_name));
    let mut input_event_type: Option<syn::Path> = None;
    let mut ctrl_select_event_type: Option<TokenStream> = None;

    // Handle input_device.
    if let Some(device_config) = input_device_config {
        input_event_type = Some(device_config.event_type.clone());
        use_statements.push(quote! { use ::rmk::event::publish_input_event_async; });
        use_statements.push(quote! { use ::rmk::input_device::InputDevice; });

        if let Some(ref enum_name) = select_enum_name {
            select_arms.push(quote! {
                event = self.read_event().fuse() => #enum_name::Input(event)
            });
            select_match_arms.push(quote! {
                #enum_name::Input(event) => {
                    publish_input_event_async(event).await;
                }
            });
        } else {
            select_arms.push(quote! {
                event = self.read_event().fuse() => {
                    publish_input_event_async(event).await;
                }
            });
        }
    }

    // Handle input_processor.
    if let Some(processor_config) = input_processor_config {
        let has_single_proc_event = processor_config.event_types.len() == 1;
        let proc_enum_name = format_ident!("{}InputEventEnum", struct_name);
        let proc_variant_names = generate_unique_variant_names(&processor_config.event_types);
        use_statements.push(quote! { use ::rmk::event::SubscribableInputEvent; });
        use_statements.push(quote! { use ::rmk::input_device::InputProcessor; });

        for (idx, (event_type, variant_name)) in processor_config
            .event_types
            .iter()
            .zip(&proc_variant_names)
            .enumerate()
        {
            let sub_name = format_ident!("proc_sub{}", idx);
            sub_defs.push(quote! {
                let mut #sub_name = <#event_type as ::rmk::event::SubscribableInputEvent>::input_subscriber();
            });

            // For single event, pass the event directly; for multiple events, wrap in enum
            let process_call = if has_single_proc_event {
                quote! { self.process(proc_event).await; }
            } else {
                quote! { self.process(#proc_enum_name::#variant_name(proc_event)).await; }
            };

            select_arms.push(quote! {
                proc_event = #sub_name.next_event().fuse() => {
                    #process_call
                }
            });
        }
    }

    // Handle controller.
    let has_polling = controller_config
        .as_ref()
        .and_then(|c| c.poll_interval_ms)
        .is_some();

    if let Some(ctrl_config) = controller_config {
        let has_single_ctrl_event = ctrl_config.event_types.len() == 1;
        let ctrl_enum =
            (!has_single_ctrl_event).then(|| format_ident!("{}ControllerEventEnum", struct_name));
        let ctrl_variant_names = generate_unique_variant_names(&ctrl_config.event_types);

        ctrl_select_event_type = Some(if has_single_ctrl_event {
            let ctrl_event_type = &ctrl_config.event_types[0];
            quote! { #ctrl_event_type }
        } else {
            let ctrl_enum = ctrl_enum.as_ref().unwrap();
            quote! { #ctrl_enum }
        });

        use_statements.push(quote! { use ::rmk::event::SubscribableControllerEvent; });
        use_statements.push(quote! { use ::rmk::controller::Controller; });

        for (idx, (ctrl_event_type, variant_name)) in ctrl_config
            .event_types
            .iter()
            .zip(&ctrl_variant_names)
            .enumerate()
        {
            let sub_name = format_ident!("ctrl_sub{}", idx);
            sub_defs.push(quote! {
                let mut #sub_name = <#ctrl_event_type as ::rmk::event::SubscribableControllerEvent>::controller_subscriber();
            });

            let wrapped_ctrl_event = if has_single_ctrl_event {
                quote! { ctrl_event }
            } else {
                let ctrl_enum = ctrl_enum.as_ref().unwrap();
                quote! { #ctrl_enum::#variant_name(ctrl_event) }
            };

            if let Some(ref enum_name) = select_enum_name {
                select_arms.push(quote! {
                    ctrl_event = #sub_name.next_event().fuse() => #enum_name::Controller(#wrapped_ctrl_event)
                });
            } else {
                select_arms.push(quote! {
                    ctrl_event = #sub_name.next_event().fuse() => {
                        <Self as ::rmk::controller::Controller>::process_event(self, #wrapped_ctrl_event).await;
                    }
                });
            }
        }

        if let Some(ref enum_name) = select_enum_name {
            select_match_arms.push(quote! {
                #enum_name::Controller(event) => {
                    <Self as ::rmk::controller::Controller>::process_event(self, event).await;
                }
            });
        }
    }

    // === Standalone cases (early returns) ===

    // Standalone controller
    if input_device_config.is_none()
        && input_processor_config.is_none()
        && controller_config.is_some()
    {
        return wrap_runnable(if has_polling {
            quote! {
                use ::rmk::controller::PollingController;
                self.polling_loop().await
            }
        } else {
            quote! {
                use ::rmk::controller::EventController;
                self.event_loop().await
            }
        });
    }

    // Standalone input_device
    if input_device_config.is_some()
        && input_processor_config.is_none()
        && controller_config.is_none()
    {
        return wrap_runnable(quote! {
            use ::rmk::event::publish_input_event_async;
            use ::rmk::input_device::InputDevice;
            loop {
                let event = self.read_event().await;
                publish_input_event_async(event).await;
            }
        });
    }

    // Standalone input_processor
    if input_device_config.is_none()
        && controller_config.is_none()
        && input_processor_config.is_some()
    {
        return wrap_runnable(quote! {
            use ::rmk::input_device::InputProcessor;
            self.process_loop().await
        });
    }

    // === Combined cases ===

    // Add common use statements
    if !sub_defs.is_empty() {
        use_statements.push(quote! { use ::rmk::event::EventSubscriber; });
    }
    use_statements.push(quote! { use ::rmk::futures::FutureExt; });

    // Build select enum definition if needed
    let select_enum_def = select_enum_name.as_ref().map(|enum_name| {
        let input_type = input_event_type.as_ref().unwrap();
        let ctrl_type = ctrl_select_event_type.as_ref().unwrap();
        if has_polling {
            quote! {
                enum #enum_name {
                    Input(#input_type),
                    Controller(#ctrl_type),
                    Timer,
                }
            }
        } else {
            quote! {
                enum #enum_name {
                    Input(#input_type),
                    Controller(#ctrl_type),
                }
            }
        }
    });

    // Build loop body
    let loop_body = if has_polling {
        let interval_ms = controller_config
            .as_ref()
            .unwrap()
            .poll_interval_ms
            .unwrap();
        use_statements.push(quote! { use ::rmk::controller::PollingController; });

        let (timer_arm, match_arms) = if let Some(ref enum_name) = select_enum_name {
            select_match_arms.push(quote! {
                #enum_name::Timer => {
                    <Self as PollingController>::update(self).await;
                    last = ::embassy_time::Instant::now();
                }
            });
            (
                quote! { _ = timer.fuse() => #enum_name::Timer },
                Some(select_match_arms.as_slice()),
            )
        } else {
            (
                quote! {
                    _ = timer.fuse() => {
                        <Self as PollingController>::update(self).await;
                        last = ::embassy_time::Instant::now();
                    }
                },
                None,
            )
        };

        build_polling_loop_body(interval_ms, timer_arm, &select_arms, match_arms)
    } else {
        let match_arms = needs_split_select.then_some(select_match_arms.as_slice());
        build_select_loop_body(&select_arms, match_arms)
    };

    // Build timer init if polling
    let timer_init = has_polling.then(|| quote! { let mut last = ::embassy_time::Instant::now(); });

    // Assemble final output
    wrap_runnable(quote! {
        #(#use_statements)*
        #select_enum_def
        #(#sub_defs)*
        #timer_init
        loop {
            #loop_body
        }
    })
}

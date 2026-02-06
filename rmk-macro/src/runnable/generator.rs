//! Unified Runnable trait implementation generator.
//!
//! Generates `Runnable` implementations for structs that combine
//! input_device, input_processor, and controller behaviors.

use proc_macro2::TokenStream;
use quote::{format_ident, quote};

use crate::controller::config::ControllerConfig;
use crate::input::config::{InputDeviceConfig, InputProcessorConfig};
use crate::utils::deduplicate_type_generics;

use super::naming::generate_unique_variant_names;

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
    let runnable_impl = |body: TokenStream| {
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
    let select_enum_name = if needs_split_select {
        Some(format_ident!("__RmkSelectEvent{}", struct_name))
    } else {
        None
    };
    let mut input_event_type: Option<syn::Path> = None;
    let mut ctrl_select_event_type: Option<TokenStream> = None;

    // Handle input_device.
    if let Some(device_config) = input_device_config {
        input_event_type = Some(device_config.event_type.clone());
        use_statements.push(quote! { use ::rmk::event::publish_input_event_async; });
        use_statements.push(quote! { use ::rmk::input_device::InputDevice; });
        if needs_split_select {
            let select_enum_name = select_enum_name.as_ref().unwrap();
            select_arms.push(quote! {
                event = self.read_event().fuse() => #select_enum_name::Input(event)
            });
            select_match_arms.push(quote! {
                #select_enum_name::Input(event) => {
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
        let proc_enum_name = format_ident!("{}EventEnum", struct_name);
        let proc_variant_names = generate_unique_variant_names(&processor_config.event_types);
        use_statements.push(quote! { use ::rmk::event::InputSubscribeEvent; });
        use_statements.push(quote! { use ::rmk::input_device::InputProcessor; });

        for (idx, (event_type, variant_name)) in processor_config
            .event_types
            .iter()
            .zip(&proc_variant_names)
            .enumerate()
        {
            let sub_name = format_ident!("proc_sub{}", idx);
            sub_defs.push(quote! {
                let mut #sub_name = <#event_type as ::rmk::event::InputSubscribeEvent>::input_subscriber();
            });
            select_arms.push(quote! {
                proc_event = #sub_name.next_event().fuse() => {
                    self.process(#proc_enum_name::#variant_name(proc_event)).await;
                }
            });
        }
    }

    // Handle controller.
    let has_polling = controller_config.as_ref().and_then(|c| c.poll_interval_ms).is_some();

    if let Some(ctrl_config) = controller_config {
        let has_single_ctrl_event = ctrl_config.event_types.len() == 1;
        let ctrl_enum = (!has_single_ctrl_event).then(|| format_ident!("{}EventEnum", struct_name));
        let ctrl_variant_names = generate_unique_variant_names(&ctrl_config.event_types);
        if has_single_ctrl_event {
            let ctrl_event_type = &ctrl_config.event_types[0];
            ctrl_select_event_type = Some(quote! { #ctrl_event_type });
        } else {
            let ctrl_enum = ctrl_enum.as_ref().unwrap();
            ctrl_select_event_type = Some(quote! { #ctrl_enum });
        }
        use_statements.push(quote! { use ::rmk::event::ControllerSubscribeEvent; });
        use_statements.push(quote! { use ::rmk::controller::Controller; });

        for (idx, (ctrl_event_type, variant_name)) in ctrl_config
            .event_types
            .iter()
            .zip(&ctrl_variant_names)
            .enumerate()
        {
            let sub_name = format_ident!("ctrl_sub{}", idx);
            sub_defs.push(quote! {
                let mut #sub_name = <#ctrl_event_type as ::rmk::event::ControllerSubscribeEvent>::controller_subscriber();
            });
            let wrapped_ctrl_event = if has_single_ctrl_event {
                quote! { ctrl_event }
            } else {
                let ctrl_enum = ctrl_enum.as_ref().unwrap();
                quote! { #ctrl_enum::#variant_name(ctrl_event) }
            };
            if needs_split_select {
                let select_enum_name = select_enum_name.as_ref().unwrap();
                select_arms.push(quote! {
                    ctrl_event = #sub_name.next_event().fuse() => #select_enum_name::Controller(#wrapped_ctrl_event)
                });
            } else {
                select_arms.push(quote! {
                    ctrl_event = #sub_name.next_event().fuse() => {
                        <Self as ::rmk::controller::Controller>::process_event(self, #wrapped_ctrl_event).await;
                    }
                });
            }
        }

        if needs_split_select {
            let select_enum_name = select_enum_name.as_ref().unwrap();
            select_match_arms.push(quote! {
                #select_enum_name::Controller(event) => {
                    <Self as ::rmk::controller::Controller>::process_event(self, event).await;
                }
            });
        }
    }

    // Standalone controller (no input_device/input_processor).
    // Now we can simply call event_loop() or polling_loop() since EventSubscriber handles the select.
    if input_device_config.is_none() && input_processor_config.is_none() && controller_config.is_some() {
        if has_polling {
            return runnable_impl(quote! {
                use ::rmk::controller::PollingController;
                self.polling_loop().await
            });
        } else {
            return runnable_impl(quote! {
                use ::rmk::controller::EventController;
                self.event_loop().await
            });
        }
    }

    // Standalone input_device.
    if input_device_config.is_some() && input_processor_config.is_none() && controller_config.is_none() {
        return runnable_impl(quote! {
            use ::rmk::event::publish_input_event_async;
            use ::rmk::input_device::InputDevice;
            loop {
                let event = self.read_event().await;
                publish_input_event_async(event).await;
            }
        });
    }

    // Standalone input_processor.
    // Now we can simply call process_loop() since EventSubscriber handles the select.
    if input_device_config.is_none() && controller_config.is_none() && input_processor_config.is_some() {
        return runnable_impl(quote! {
            use ::rmk::input_device::InputProcessor;
            self.process_loop().await
        });
    }

    // Common use statements.
    if !sub_defs.is_empty() {
        use_statements.push(quote! { use ::rmk::event::EventSubscriber; });
    }
    use_statements.push(quote! { use ::rmk::futures::FutureExt; });

    // Generate polling-related code if needed.
    if has_polling {
        let interval_ms = controller_config.as_ref().unwrap().poll_interval_ms.unwrap();
        use_statements.push(quote! { use ::rmk::controller::PollingController; });

        let timer_init = quote! {
            let mut last = ::embassy_time::Instant::now();
        };

        if needs_split_select {
            let select_enum_name = select_enum_name.as_ref().unwrap();
            let input_event_type = input_event_type.as_ref().unwrap();
            let ctrl_event_type = ctrl_select_event_type.as_ref().unwrap();
            let select_enum_def = quote! {
                enum #select_enum_name {
                    Input(#input_event_type),
                    Controller(#ctrl_event_type),
                    Timer,
                }
            };

            let timer_arm = quote! {
                _ = timer.fuse() => #select_enum_name::Timer
            };

            select_match_arms.push(quote! {
                #select_enum_name::Timer => {
                    <Self as PollingController>::update(self).await;
                    last = ::embassy_time::Instant::now();
                }
            });

            quote! {
                impl #impl_generics ::rmk::input_device::Runnable for #struct_name #ty_generics #where_clause {
                    async fn run(&mut self) -> ! {
                        #(#use_statements)*
                        #select_enum_def

                        #(#sub_defs)*
                        #timer_init

                        loop {
                            let elapsed = last.elapsed();
                            let interval = ::embassy_time::Duration::from_millis(#interval_ms);
                            let timer = ::embassy_time::Timer::after(
                                interval.checked_sub(elapsed).unwrap_or(::embassy_time::Duration::MIN)
                            );

                            let select_result = {
                                ::rmk::futures::select_biased! {
                                    #timer_arm,
                                    #(#select_arms),*
                                }
                            };

                            match select_result {
                                #(#select_match_arms)*
                            }
                        }
                    }
                }
            }
        } else {
            let timer_arm = quote! {
                _ = timer.fuse() => {
                    <Self as PollingController>::update(self).await;
                    last = ::embassy_time::Instant::now();
                }
            };

            quote! {
                impl #impl_generics ::rmk::input_device::Runnable for #struct_name #ty_generics #where_clause {
                    async fn run(&mut self) -> ! {
                        #(#use_statements)*

                        #(#sub_defs)*
                        #timer_init

                        loop {
                            let elapsed = last.elapsed();
                            let interval = ::embassy_time::Duration::from_millis(#interval_ms);
                            let timer = ::embassy_time::Timer::after(
                                interval.checked_sub(elapsed).unwrap_or(::embassy_time::Duration::MIN)
                            );

                            ::rmk::futures::select_biased! {
                                #timer_arm,
                                #(#select_arms),*
                            }
                        }
                    }
                }
            }
        }
    } else if needs_split_select {
        let select_enum_name = select_enum_name.as_ref().unwrap();
        let input_event_type = input_event_type.as_ref().unwrap();
        let ctrl_event_type = ctrl_select_event_type.as_ref().unwrap();
        let select_enum_def = quote! {
            enum #select_enum_name {
                Input(#input_event_type),
                Controller(#ctrl_event_type),
            }
        };

        quote! {
            impl #impl_generics ::rmk::input_device::Runnable for #struct_name #ty_generics #where_clause {
                async fn run(&mut self) -> ! {
                    #(#use_statements)*
                    #select_enum_def

                    #(#sub_defs)*

                    loop {
                        let select_result = {
                            ::rmk::futures::select_biased! {
                                #(#select_arms),*
                            }
                        };

                        match select_result {
                            #(#select_match_arms)*
                        }
                    }
                }
            }
        }
    } else {
        quote! {
            impl #impl_generics ::rmk::input_device::Runnable for #struct_name #ty_generics #where_clause {
                async fn run(&mut self) -> ! {
                    #(#use_statements)*

                    #(#sub_defs)*

                    loop {
                        ::rmk::futures::select_biased! {
                            #(#select_arms),*
                        }
                    }
                }
            }
        }
    }
}

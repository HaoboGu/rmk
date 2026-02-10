//! Unified Runnable trait implementation generator.
//!
//! Generates `Runnable` implementations for structs that combine
//! input_device and processor behaviors.

use proc_macro2::TokenStream;
use quote::{format_ident, quote};

use crate::event_macros::config::InputDeviceConfig;
use crate::event_macros::utils::deduplicate_type_generics;
use crate::processor::ProcessorConfig;

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

/// Check if a path matches any path in a list (by last segment ident).
fn path_in_list(needle: &syn::Path, haystack: &[syn::Path]) -> bool {
    let needle_ident = needle.segments.last().map(|s| &s.ident);
    haystack
        .iter()
        .any(|p| p.segments.last().map(|s| &s.ident) == needle_ident)
}

/// Generate a unified `Runnable` impl for input_device and/or processor.
///
/// Handles:
/// - InputDevice: read_event + publish
/// - Processor: subscribe + process + optional polling
///
/// Uses select_biased! when multiplexing multiple sources.
///
/// # Panics
/// Panics at compile time if the same event type is both published and subscribed,
/// which would cause a self-deadlock.
pub fn generate_runnable(
    struct_name: &syn::Ident,
    generics: &syn::Generics,
    where_clause: Option<&syn::WhereClause>,
    input_device_config: Option<&InputDeviceConfig>,
    processor_config: Option<&ProcessorConfig>,
) -> TokenStream {
    // Check for self-deadlock: published event type must not be in subscribe list of the same struct
    if let (Some(input_cfg), Some(proc_cfg)) = (input_device_config, processor_config) {
        let publish_type = &input_cfg.event_type;
        if path_in_list(publish_type, &proc_cfg.event_types) {
            panic!(
                "Self-deadlock detected on `{}`: cannot publish and subscribe the same event type `{}`. \
                The task would block on publish_event_async() while the only consumer is in the same blocked task.",
                struct_name,
                quote! { #publish_type }
            );
        }
    }

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

    // Collect select arms and subscriber definitions.
    let mut sub_defs: Vec<TokenStream> = Vec::new();
    let mut select_arms: Vec<TokenStream> = Vec::new();
    let mut select_match_arms: Vec<TokenStream> = Vec::new();
    let mut use_statements: Vec<TokenStream> = Vec::new();

    let needs_split_select = input_device_config.is_some() && processor_config.is_some();
    let select_enum_name =
        needs_split_select.then(|| format_ident!("__RmkSelectEvent{}", struct_name));
    let mut input_event_type: Option<syn::Path> = None;

    // Handle input_device.
    if let Some(device_config) = input_device_config {
        input_event_type = Some(device_config.event_type.clone());
        use_statements.push(quote! { use ::rmk::event::publish_event_async; });
        use_statements.push(quote! { use ::rmk::input_device::InputDevice; });

        if let Some(ref enum_name) = select_enum_name {
            select_arms.push(quote! {
                event = self.read_event().fuse() => #enum_name::Input(event)
            });
            select_match_arms.push(quote! {
                #enum_name::Input(event) => {
                    publish_event_async(event).await;
                }
            });
        } else {
            select_arms.push(quote! {
                event = self.read_event().fuse() => {
                    publish_event_async(event).await;
                }
            });
        }
    }

    // Handle processor.
    let has_polling = processor_config
        .as_ref()
        .and_then(|c| c.poll_interval_ms)
        .is_some();

    if processor_config.is_some() {
        use_statements.push(quote! { use ::rmk::event::SubscribableEvent; });
        use_statements.push(quote! { use ::rmk::processor::Processor; });

        sub_defs.push(quote! {
            let mut proc_sub = <Self as ::rmk::processor::Processor>::subscriber();
        });

        if let Some(ref enum_name) = select_enum_name {
            select_arms.push(quote! {
                proc_event = proc_sub.next_event().fuse() => #enum_name::Processor(proc_event)
            });
        } else {
            select_arms.push(quote! {
                proc_event = proc_sub.next_event().fuse() => {
                    <Self as ::rmk::processor::Processor>::process(self, proc_event).await;
                }
            });
        }

        if let Some(ref enum_name) = select_enum_name {
            select_match_arms.push(quote! {
                #enum_name::Processor(event) => {
                    <Self as ::rmk::processor::Processor>::process(self, event).await;
                }
            });
        }
    }

    // === Standalone cases (early returns) ===

    // Standalone processor
    if input_device_config.is_none() && processor_config.is_some() {
        return wrap_runnable(if has_polling {
            quote! {
                use ::rmk::processor::PollingProcessor;
                self.polling_loop().await
            }
        } else {
            quote! {
                use ::rmk::processor::Processor;
                self.process_loop().await
            }
        });
    }

    // Standalone input_device
    if input_device_config.is_some() && processor_config.is_none() {
        return wrap_runnable(quote! {
            use ::rmk::event::publish_event_async;
            use ::rmk::input_device::InputDevice;
            loop {
                let event = self.read_event().await;
                publish_event_async(event).await;
            }
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
        // Use the Processor trait's associated Event type
        let proc_type = quote! { <Self as ::rmk::processor::Processor>::Event };
        if has_polling {
            quote! {
                enum #enum_name {
                    Input(#input_type),
                    Processor(#proc_type),
                    Timer,
                }
            }
        } else {
            quote! {
                enum #enum_name {
                    Input(#input_type),
                    Processor(#proc_type),
                }
            }
        }
    });

    // Build loop body
    let loop_body = if has_polling {
        let interval_ms = processor_config.as_ref().unwrap().poll_interval_ms.unwrap();
        use_statements.push(quote! { use ::rmk::processor::PollingProcessor; });

        let (timer_arm, match_arms) = if let Some(ref enum_name) = select_enum_name {
            select_match_arms.push(quote! {
                #enum_name::Timer => {
                    <Self as ::rmk::processor::PollingProcessor>::update(self).await;
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
                        <Self as ::rmk::processor::PollingProcessor>::update(self).await;
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

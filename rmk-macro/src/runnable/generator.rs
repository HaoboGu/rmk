//! Unified Runnable trait implementation generator.
//!
//! Generates `Runnable` implementations for structs that combine
//! input_device and processor behaviors.

use proc_macro2::TokenStream;
use quote::quote;

use crate::input::config::InputDeviceConfig;
use crate::processor::ProcessorConfig;
use crate::utils::deduplicate_type_generics;

/// Build the loop body for select.
fn build_select_loop_body(select_arms: &[TokenStream]) -> TokenStream {
    quote! {
        ::rmk::futures::select_biased! {
            #(#select_arms),*
        }
    }
}

/// Build the polling loop body with timer.
fn build_polling_loop_body(
    interval_ms: u64,
    timer_arm: TokenStream,
    select_arms: &[TokenStream],
) -> TokenStream {
    quote! {
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

/// Generate a unified `Runnable` impl for input_device and/or processor.
///
/// Handles:
/// - InputDevice: read_event + publish
/// - Processor: subscribe + process + optional polling
///
/// Uses select_biased! when multiplexing multiple sources.
pub fn generate_runnable(
    struct_name: &syn::Ident,
    generics: &syn::Generics,
    where_clause: Option<&syn::WhereClause>,
    input_device_config: Option<&InputDeviceConfig>,
    processor_config: Option<&ProcessorConfig>,
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

    let has_polling = processor_config
        .as_ref()
        .and_then(|c| c.poll_interval_ms)
        .is_some();

    // Collect select arms and subscriber definitions.
    let mut sub_defs: Vec<TokenStream> = Vec::new();
    let mut select_arms: Vec<TokenStream> = Vec::new();
    let mut use_statements: Vec<TokenStream> = Vec::new();

    // Handle input_device.
    if input_device_config.is_some() {
        use_statements.push(quote! { use ::rmk::event::publish_event_async; });
        use_statements.push(quote! { use ::rmk::input_device::InputDevice; });

        select_arms.push(quote! {
            event = self.read_event().fuse() => {
                publish_event_async(event).await;
            }
        });
    }

    // Handle processor.
    if processor_config.is_some() {
        use_statements.push(quote! { use ::rmk::event::SubscribableEvent; });
        use_statements.push(quote! { use ::rmk::processor::Processor; });

        sub_defs.push(quote! {
            let mut proc_sub = <Self as ::rmk::processor::Processor>::subscriber();
        });

        select_arms.push(quote! {
            proc_event = proc_sub.next_event().fuse() => {
                <Self as ::rmk::processor::Processor>::process(self, proc_event).await;
            }
        });
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

    // === Combined cases (input_device + processor) ===

    // Add common use statements
    if !sub_defs.is_empty() {
        use_statements.push(quote! { use ::rmk::event::EventSubscriber; });
    }
    use_statements.push(quote! { use ::rmk::futures::FutureExt; });

    // Build loop body
    let loop_body = if has_polling {
        let interval_ms = processor_config
            .as_ref()
            .unwrap()
            .poll_interval_ms
            .unwrap();
        use_statements.push(quote! { use ::rmk::processor::PollingProcessor; });

        let timer_arm = quote! {
            _ = timer.fuse() => {
                <Self as PollingProcessor>::update(self).await;
                last = ::embassy_time::Instant::now();
            }
        };

        build_polling_loop_body(interval_ms, timer_arm, &select_arms)
    } else {
        build_select_loop_body(&select_arms)
    };

    // Build timer init if polling
    let timer_init = has_polling.then(|| quote! { let mut last = ::embassy_time::Instant::now(); });

    // Assemble final output
    wrap_runnable(quote! {
        #(#use_statements)*
        #(#sub_defs)*
        #timer_init
        loop {
            #loop_body
        }
    })
}

use rmk_macro::input_processor;
pub struct KeyEvent {
    pub row: u8,
    pub col: u8,
    pub pressed: bool,
}
#[automatically_derived]
#[doc(hidden)]
unsafe impl ::core::clone::TrivialClone for KeyEvent {}
#[automatically_derived]
impl ::core::clone::Clone for KeyEvent {
    #[inline]
    fn clone(&self) -> KeyEvent {
        let _: ::core::clone::AssertParamIsClone<u8>;
        let _: ::core::clone::AssertParamIsClone<bool>;
        *self
    }
}
#[automatically_derived]
impl ::core::marker::Copy for KeyEvent {}
#[automatically_derived]
impl ::core::fmt::Debug for KeyEvent {
    #[inline]
    fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
        ::core::fmt::Formatter::debug_struct_field3_finish(
            f,
            "KeyEvent",
            "row",
            &self.row,
            "col",
            &self.col,
            "pressed",
            &&self.pressed,
        )
    }
}
pub struct SingleEventInputProcessor;
impl ::rmk::input_device::Runnable for SingleEventInputProcessor {
    async fn run(&mut self) -> ! {
        use ::rmk::input_device::InputProcessor;
        self.process_loop().await
    }
}
impl ::rmk::input_device::InputProcessor for SingleEventInputProcessor {
    type Event = KeyEvent;
    async fn process(&mut self, event: Self::Event) {
        self.on_key_event(event).await
    }
}

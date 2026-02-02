use rmk_macro::InputEvent;
pub struct BatteryEvent {
    pub level: u8,
}
#[automatically_derived]
impl ::core::clone::Clone for BatteryEvent {
    #[inline]
    fn clone(&self) -> BatteryEvent {
        let _: ::core::clone::AssertParamIsClone<u8>;
        *self
    }
}
#[automatically_derived]
impl ::core::marker::Copy for BatteryEvent {}
#[automatically_derived]
impl ::core::fmt::Debug for BatteryEvent {
    #[inline]
    fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
        ::core::fmt::Formatter::debug_struct_field1_finish(
            f,
            "BatteryEvent",
            "level",
            &&self.level,
        )
    }
}
pub struct PointingEvent {
    pub x: i16,
    pub y: i16,
}
#[automatically_derived]
impl ::core::clone::Clone for PointingEvent {
    #[inline]
    fn clone(&self) -> PointingEvent {
        let _: ::core::clone::AssertParamIsClone<i16>;
        *self
    }
}
#[automatically_derived]
impl ::core::marker::Copy for PointingEvent {}
#[automatically_derived]
impl ::core::fmt::Debug for PointingEvent {
    #[inline]
    fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
        ::core::fmt::Formatter::debug_struct_field2_finish(
            f,
            "PointingEvent",
            "x",
            &self.x,
            "y",
            &&self.y,
        )
    }
}
pub enum MultiSensorEvent {
    Battery(BatteryEvent),
    Pointing(PointingEvent),
}
impl MultiSensorEvent {
    /// Publish this event to the appropriate channel based on variant
    pub async fn publish(self) {
        match self {
            MultiSensorEvent::Battery(e) => {
                ::rmk::event::publish_input_event_async(e).await
            }
            MultiSensorEvent::Pointing(e) => {
                ::rmk::event::publish_input_event_async(e).await
            }
        }
    }
}
impl From<BatteryEvent> for MultiSensorEvent {
    fn from(e: BatteryEvent) -> Self {
        MultiSensorEvent::Battery(e)
    }
}
impl From<PointingEvent> for MultiSensorEvent {
    fn from(e: PointingEvent) -> Self {
        MultiSensorEvent::Pointing(e)
    }
}

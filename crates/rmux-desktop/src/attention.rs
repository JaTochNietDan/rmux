//! Unacknowledged alert sources. Dismissing a message only acknowledges that message.
#[derive(Clone, Copy, Debug, Default)]
pub struct Attention {
    notification: bool,
    bell: bool,
    status: bool,
}

impl Attention {
    pub fn any(self) -> bool { self.notification || self.bell || self.status }
    pub fn notify(&mut self) { self.notification = true; }
    pub fn bell(&mut self) { self.bell = true; }
    pub fn status(&mut self) { self.status = true; }
    pub fn dismiss_notification(&mut self) { self.notification = false; }
    pub fn acknowledge(&mut self) { *self = Self::default(); }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dismissing_messages_preserves_other_alerts_in_either_order() {
        for bell in [false, true] {
            for status in [false, true] {
                for notification_first in [false, true] {
                    let mut attention = Attention::default();
                    if notification_first { attention.notify(); }
                    if bell { attention.bell(); }
                    if status { attention.status(); }
                    if !notification_first { attention.notify(); }
                    assert!(attention.any());
                    attention.dismiss_notification();
                    assert_eq!(attention.any(), bell || status);
                    attention.dismiss_notification();
                    assert_eq!(attention.any(), bell || status);
                    attention.acknowledge();
                    assert!(!attention.any());
                    attention.notify();
                    assert!(attention.any());
                    attention.dismiss_notification();
                    assert!(!attention.any());
                }
            }
        }
    }
}

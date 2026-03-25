use ratatui::style::Color;

pub const RAVEN:    Color = Color::Rgb(11, 12, 13);
pub const COAL:     Color = Color::Rgb(20, 22, 24);
pub const GRAPHITE: Color = Color::Rgb(33, 37, 41);
pub const BONE:     Color = Color::Rgb(234, 230, 221);
pub const MIST:     Color = Color::Rgb(167, 170, 164);
pub const ASH:      Color = Color::Rgb(93, 97, 93);

pub const WORKING:  Color = Color::Rgb(29, 158, 117);
pub const WAITING:  Color = Color::Rgb(239, 159, 39);
pub const IDLE:     Color = Color::Rgb(136, 135, 128);
pub const DEAD:     Color = Color::Rgb(226, 75, 74);

pub fn status_color(status: &caw_core::SessionStatus) -> Color {
    match status {
        caw_core::SessionStatus::Working      => WORKING,
        caw_core::SessionStatus::WaitingInput => WAITING,
        caw_core::SessionStatus::Idle         => IDLE,
        caw_core::SessionStatus::Dead         => DEAD,
    }
}

use std::thread;
use std::time::Duration;

use anyhow::Result;
use enigo::{Enigo, MouseButton, MouseControllable};

use crate::geometry::ScreenPoint;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InputPlan {
    pub from: ScreenPoint,
    pub to: ScreenPoint,
    pub delay: Duration,
}

pub trait InputInjector {
    fn click(&mut self, point: ScreenPoint) -> Result<()>;

    fn click_move(&mut self, plan: &InputPlan) -> Result<()> {
        self.click(plan.from)?;
        thread::sleep(plan.delay);
        self.click(plan.to)?;
        Ok(())
    }
}

pub struct EnigoInjector {
    enigo: Enigo,
}

impl EnigoInjector {
    #[must_use]
    pub fn new() -> Self {
        Self { enigo: Enigo::new() }
    }
}

impl Default for EnigoInjector {
    fn default() -> Self {
        Self::new()
    }
}

impl InputInjector for EnigoInjector {
    fn click(&mut self, point: ScreenPoint) -> Result<()> {
        self.enigo.mouse_move_to(point.x, point.y);
        self.enigo.mouse_click(MouseButton::Left);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestInjector {
        clicks: Vec<ScreenPoint>,
    }

    impl TestInjector {
        fn new() -> Self {
            Self { clicks: Vec::new() }
        }
    }

    impl InputInjector for TestInjector {
        fn click(&mut self, point: ScreenPoint) -> Result<()> {
            self.clicks.push(point);
            Ok(())
        }
    }

    #[test]
    fn click_move_orders_points() {
        let mut injector = TestInjector::new();
        let plan = InputPlan {
            from: ScreenPoint { x: 1, y: 2 },
            to: ScreenPoint { x: 3, y: 4 },
            delay: Duration::from_millis(0),
        };
        injector.click_move(&plan).expect("should work");
        assert_eq!(injector.clicks.len(), 2);
        assert_eq!(injector.clicks[0], plan.from);
        assert_eq!(injector.clicks[1], plan.to);
    }
}

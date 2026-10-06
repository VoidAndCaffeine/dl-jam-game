use bevy::prelude::*;

/// A short-lived impact spark, drawn as a red core inside a yellow fringe.
#[derive(Component, Reflect, Debug, Clone, Copy)]
pub struct HitSpark {
    pub remaining: f32,
    pub total: f32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_spark_remembers_its_lifetime() {
        let spark = HitSpark {
            remaining: 0.05,
            total: 0.09,
        };
        assert!(spark.remaining <= spark.total);
    }
}

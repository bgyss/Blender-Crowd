//! Property tests, contract section 15.1.

use crowd_core::geometry::{time_to_collision_disc, Segment};
use crowd_core::grid::UniformGrid;
use crowd_core::ids::derive_agent_id;
use crowd_core::rng::{Purpose, StableRng};
use crowd_core::units::{Aabb, Vec2};
use proptest::prelude::*;

fn any_coordinate() -> impl Strategy<Value = f32> {
    -100.0f32..100.0f32
}

proptest! {
    #[test]
    fn derived_ids_are_injective_over_ordinals(
        seed in any::<u64>(),
        population in any::<u16>(),
        source in any::<u16>(),
        a in any::<u32>(),
        b in any::<u32>(),
    ) {
        prop_assume!(a != b);
        prop_assert_ne!(
            derive_agent_id(seed, population, source, a),
            derive_agent_id(seed, population, source, b)
        );
    }

    #[test]
    fn random_draws_stay_within_their_range(
        seed in any::<u64>(),
        ordinal in any::<u32>(),
        lo in -50.0f32..0.0f32,
        span in 0.1f32..50.0f32,
    ) {
        let id = derive_agent_id(seed, 0, 0, ordinal);
        let mut rng = StableRng::for_agent(seed, id, Purpose::Radius);
        let value = rng.range_f32(lo, lo + span);
        prop_assert!(value >= lo && value <= lo + span);
    }

    #[test]
    fn normal_draws_are_always_finite(seed in any::<u64>()) {
        let mut rng = StableRng::from_seed(seed);
        for _ in 0..64 {
            prop_assert!(rng.normal_f32(1.35, 0.18).is_finite());
        }
    }

    #[test]
    fn closest_point_is_never_further_than_either_endpoint(
        ax in any_coordinate(), ay in any_coordinate(),
        bx in any_coordinate(), by in any_coordinate(),
        px in any_coordinate(), py in any_coordinate(),
    ) {
        let seg = Segment::new(Vec2::new(ax, ay), Vec2::new(bx, by));
        let p = Vec2::new(px, py);
        let d = seg.distance_to(p);
        prop_assert!(d <= (p - seg.a).length() + 1e-3);
        prop_assert!(d <= (p - seg.b).length() + 1e-3);
    }

    #[test]
    fn time_to_collision_is_never_negative(
        px in any_coordinate(), py in any_coordinate(),
        vx in -5.0f32..5.0f32, vy in -5.0f32..5.0f32,
        radius in 0.1f32..2.0f32,
    ) {
        if let Some(t) = time_to_collision_disc(Vec2::new(px, py), Vec2::new(vx, vy), radius) {
            prop_assert!(t >= 0.0);
            prop_assert!(t.is_finite());
        }
    }

    #[test]
    fn grid_queries_never_miss_a_point_brute_force_finds(
        xs in prop::collection::vec(-40.0f32..40.0f32, 1..80),
        radius in 0.5f32..8.0f32,
    ) {
        let ys: Vec<f32> = xs.iter().map(|x| x * 0.37).collect();
        let bounds = Aabb::new(Vec2::new(-50.0, -50.0), Vec2::new(50.0, 50.0));
        let mut grid = UniformGrid::new(bounds, 4.0);
        grid.rebuild(&xs, &ys);

        let centre = Vec2::new(xs[0], ys[0]);
        let mut found = Vec::new();
        grid.query(centre, radius, &mut found);

        for i in 0..xs.len() {
            let d = Vec2::new(xs[i], ys[i]).distance_squared(centre);
            if d <= radius * radius {
                prop_assert!(
                    found.contains(&(i as u32)),
                    "grid missed slot {i} at distance {}",
                    d.sqrt()
                );
            }
        }
    }
}

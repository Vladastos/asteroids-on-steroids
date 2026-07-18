//! Bevy collision system ported from C# `Engine/Systems/CollisionSystem.cs`.

use std::collections::HashSet;

use bevy::prelude::*;

use crate::components::{Collider, DisabledTag, RigidBody, Velocity};

pub mod game_layers {
    pub const ASTEROID: i32 = 1;
    pub const PLAYER: i32 = 2;
    pub const ALIEN: i32 = 4;
    pub const BULLET: i32 = 8;
    pub const GHOST: i32 = 16;
}

#[derive(Event, Clone, Copy, Debug)]
pub struct CollisionEvent {
    pub entity_a: Entity,
    pub entity_b: Entity,
    pub contact: collision::ContactInfo,
    pub approach_speed: f32,
}

#[derive(Resource, Default)]
pub struct CollisionGrid {
    pub spatial: collision::SpatialGrid<Entity>,
}

const LIN_SLEEP_TOL: f32 = 6.0;
const ANG_SLEEP_TOL: f32 = 0.12;
const SLEEP_TIME: f32 = 0.5;
const RESTITUTION_VEL_THRESHOLD: f32 = 30.0;
const PENETRATION_SLOP: f32 = 0.5;
const CORRECTION_PERCENT: f32 = 0.4;
const MAX_CORRECTION: f32 = 16.0;
const DEEP_NO_BOUNCE: f32 = 100000.0;
const ITERATIONS: usize = 6;

#[derive(Clone)]
struct ColliderSnapshot {
    entity: Entity,
    position: Vec2,
    rotation: f32,
    shape: collision::Shape,
    layer: i32,
    mask: i32,
    sensor: bool,
}

struct PairHit {
    entity_a: Entity,
    entity_b: Entity,
    contacts: Vec<collision::ContactInfo>,
    deepest: collision::ContactInfo,
    sensor: bool,
}

struct Contact {
    a: Entity,
    b: Entity,
    normal: Vec2,
    tangent: Vec2,
    r_a: Vec2,
    r_b: Vec2,
    inv_mass_a: f32,
    inv_mass_b: f32,
    inv_ia: f32,
    inv_ib: f32,
    normal_mass: f32,
    tangent_mass: f32,
    velocity_bias: f32,
    friction: f32,
    accum_n: f32,
    accum_t: f32,
}

#[allow(clippy::type_complexity)]
pub fn collision_system(
    mut grid: ResMut<CollisionGrid>,
    mut transforms: ParamSet<(
        Query<(Entity, &Transform, &Collider), Without<DisabledTag>>,
        Query<&mut Transform>,
    )>,
    mut bodies: Query<(Entity, &mut RigidBody)>,
    mut velocities: Query<&mut Velocity>,
    time: Res<Time<Fixed>>,
    mut events: EventWriter<CollisionEvent>,
) {
    grid.spatial.clear();

    let snapshots: Vec<_> = transforms
        .p0()
        .iter()
        .map(|(entity, transform, collider)| {
            let position = transform.translation.truncate();
            let rotation = transform.rotation.to_euler(EulerRot::ZYX).0;
            let (min, max) = collision::get_aabb(&collider.shape, position, rotation);
            grid.spatial.insert(entity, min, max);

            ColliderSnapshot {
                entity,
                position,
                rotation,
                shape: collider.shape.clone(),
                layer: collider.layer,
                mask: collider.mask,
                sensor: collider.sensor,
            }
        })
        .collect();

    let mut candidates = Vec::new();
    let mut tested_pairs = HashSet::new();
    let mut pair_hits = Vec::new();
    let mut hit_buf = Vec::new();

    for a in &snapshots {
        let (min, max) = collision::get_aabb(&a.shape, a.position, a.rotation);
        candidates.clear();
        grid.spatial.candidates(min, max, &mut candidates);

        for entity_b in &candidates {
            if *entity_b == a.entity {
                continue;
            }

            let pair = sorted_pair_key(a.entity, *entity_b);
            if !tested_pairs.insert(pair) {
                continue;
            }

            let Some(b) = snapshots
                .iter()
                .find(|snapshot| snapshot.entity == *entity_b)
            else {
                continue;
            };

            if (a.mask & b.layer) == 0 && (b.mask & a.layer) == 0 {
                continue;
            }

            hit_buf.clear();
            collect_pair_contacts(
                &a.shape,
                a.position,
                a.rotation,
                &b.shape,
                b.position,
                b.rotation,
                &mut hit_buf,
            );
            if hit_buf.is_empty() {
                continue;
            }

            let deepest = hit_buf
                .iter()
                .copied()
                .max_by(|lhs, rhs| lhs.depth.total_cmp(&rhs.depth))
                .expect("hit buffer is non-empty");

            pair_hits.push(PairHit {
                entity_a: a.entity,
                entity_b: b.entity,
                contacts: hit_buf.clone(),
                deepest,
                sensor: a.sensor || b.sensor,
            });
        }
    }

    let mut contacts = Vec::new();
    for hit in pair_hits {
        if !hit.sensor {
            try_separate(
                hit.entity_a,
                hit.entity_b,
                hit.deepest,
                &mut transforms.p1(),
                &bodies,
            );
            for contact in &hit.contacts {
                gather_contact(
                    hit.entity_a,
                    hit.entity_b,
                    *contact,
                    &transforms.p1(),
                    &mut bodies,
                    &velocities,
                    &mut contacts,
                );
            }
        }

        let approach = approach_normal_speed(
            hit.entity_a,
            hit.entity_b,
            hit.deepest,
            &transforms.p1(),
            &velocities,
        );
        events.write(CollisionEvent {
            entity_a: hit.entity_a,
            entity_b: hit.entity_b,
            contact: hit.deepest,
            approach_speed: approach,
        });
    }

    for _ in 0..ITERATIONS {
        for contact in &mut contacts {
            solve_contact(contact, &mut velocities);
        }
    }

    update_sleep(time.delta_secs(), &mut bodies, &mut velocities);
}

fn sorted_pair_key(a: Entity, b: Entity) -> (u32, u32) {
    let id_a = a.index();
    let id_b = b.index();
    if id_a < id_b {
        (id_a, id_b)
    } else {
        (id_b, id_a)
    }
}

fn collect_pair_contacts(
    shape_a: &collision::Shape,
    pos_a: Vec2,
    rot_a: f32,
    shape_b: &collision::Shape,
    pos_b: Vec2,
    rot_b: f32,
    out: &mut Vec<collision::ContactInfo>,
) {
    match (shape_a, shape_b) {
        (collision::Shape::Compound(compound_a), collision::Shape::Compound(compound_b)) => {
            let start = out.len();
            if compound_a.parts.len() >= compound_b.parts.len() {
                collision::collect_contacts(pos_a, rot_a, compound_a, pos_b, rot_b, shape_b, out);
                for contact in &mut out[start..] {
                    *contact = contact.flipped();
                }
            } else {
                collision::collect_contacts(pos_b, rot_b, compound_b, pos_a, rot_a, shape_a, out);
                for contact in &mut out[start..] {
                    *contact = swapped_parts(*contact);
                }
            }
        }
        (collision::Shape::Compound(compound_a), _) => {
            let start = out.len();
            collision::collect_contacts(pos_a, rot_a, compound_a, pos_b, rot_b, shape_b, out);
            for contact in &mut out[start..] {
                *contact = contact.flipped();
            }
        }
        (_, collision::Shape::Compound(compound_b)) => {
            let start = out.len();
            collision::collect_contacts(pos_b, rot_b, compound_b, pos_a, rot_a, shape_a, out);
            for contact in &mut out[start..] {
                *contact = swapped_parts(*contact);
            }
        }
        _ => {
            if let Some(mut contact) =
                collision::intersects(pos_a, rot_a, shape_a, pos_b, rot_b, shape_b)
            {
                if contact.normal.dot(pos_a - pos_b) < 0.0 {
                    contact = contact.flipped();
                }
                out.push(contact);
            }
        }
    }
}

fn swapped_parts(contact: collision::ContactInfo) -> collision::ContactInfo {
    collision::ContactInfo {
        part_a: contact.part_b,
        part_b: contact.part_a,
        ..contact
    }
}

fn try_separate(
    entity_a: Entity,
    entity_b: Entity,
    contact: collision::ContactInfo,
    transforms: &mut Query<&mut Transform>,
    bodies: &Query<(Entity, &mut RigidBody)>,
) {
    let (Ok((_, body_a)), Ok((_, body_b))) = (bodies.get(entity_a), bodies.get(entity_b)) else {
        return;
    };

    let total = body_a.mass + body_b.mass;
    let share_a = if total > 0.0 {
        body_b.mass / total
    } else {
        0.5
    };

    let mut correction = (contact.depth - PENETRATION_SLOP).max(0.0) * CORRECTION_PERCENT;
    if correction <= 0.0 {
        return;
    }
    correction = correction.min(MAX_CORRECTION);
    let correction_vec = contact.normal * correction;

    let Ok([mut transform_a, mut transform_b]) = transforms.get_many_mut([entity_a, entity_b])
    else {
        return;
    };
    transform_a.translation += (correction_vec * share_a).extend(0.0);
    transform_b.translation -= (correction_vec * (1.0 - share_a)).extend(0.0);
}

fn gather_contact(
    entity_a: Entity,
    entity_b: Entity,
    info: collision::ContactInfo,
    transforms: &Query<&mut Transform>,
    bodies: &mut Query<(Entity, &mut RigidBody)>,
    velocities: &Query<&mut Velocity>,
    contacts: &mut Vec<Contact>,
) {
    let (Ok(transform_a), Ok(transform_b)) = (transforms.get(entity_a), transforms.get(entity_b))
    else {
        return;
    };
    let (Ok(velocity_a), Ok(velocity_b)) = (velocities.get(entity_a), velocities.get(entity_b))
    else {
        return;
    };
    let Ok([(_, mut body_a), (_, mut body_b)]) = bodies.get_many_mut([entity_a, entity_b]) else {
        return;
    };

    body_a.asleep = false;
    body_a.sleep_timer = 0.0;
    body_b.asleep = false;
    body_b.sleep_timer = 0.0;

    let normal = info.normal;
    let tangent = Vec2::new(-normal.y, normal.x);
    let r_a = info.contact_point - transform_a.translation.truncate();
    let r_b = info.contact_point - transform_b.translation.truncate();

    let inv_mass_a = if body_a.mass > 0.0 {
        1.0 / body_a.mass
    } else {
        0.0
    };
    let inv_mass_b = if body_b.mass > 0.0 {
        1.0 / body_b.mass
    } else {
        0.0
    };
    let inv_ia = if body_a.inertia > 0.0 {
        1.0 / body_a.inertia
    } else {
        0.0
    };
    let inv_ib = if body_b.inertia > 0.0 {
        1.0 / body_b.inertia
    } else {
        0.0
    };

    let rn_a = cross(r_a, normal);
    let rn_b = cross(r_b, normal);
    let kn = inv_mass_a + inv_mass_b + inv_ia * rn_a * rn_a + inv_ib * rn_b * rn_b;
    let rt_a = cross(r_a, tangent);
    let rt_b = cross(r_b, tangent);
    let kt = inv_mass_a + inv_mass_b + inv_ia * rt_a * rt_a + inv_ib * rt_b * rt_b;

    let v_rel = vel_at(velocity_a, r_a) - vel_at(velocity_b, r_b);
    let vn0 = v_rel.dot(normal);
    let restitution = body_a.restitution.min(body_b.restitution);
    let velocity_bias = if vn0 < -RESTITUTION_VEL_THRESHOLD && info.depth < DEEP_NO_BOUNCE {
        -restitution * vn0
    } else {
        0.0
    };

    contacts.push(Contact {
        a: entity_a,
        b: entity_b,
        normal,
        tangent,
        r_a,
        r_b,
        inv_mass_a,
        inv_mass_b,
        inv_ia,
        inv_ib,
        normal_mass: if kn > 0.0 { 1.0 / kn } else { 0.0 },
        tangent_mass: if kt > 0.0 { 1.0 / kt } else { 0.0 },
        velocity_bias,
        friction: (body_a.friction.max(0.0) * body_b.friction.max(0.0)).sqrt(),
        accum_n: 0.0,
        accum_t: 0.0,
    });
}

fn solve_contact(contact: &mut Contact, velocities: &mut Query<&mut Velocity>) {
    let Ok([mut velocity_a, mut velocity_b]) = velocities.get_many_mut([contact.a, contact.b])
    else {
        return;
    };

    let v_rel = vel_at(&velocity_a, contact.r_a) - vel_at(&velocity_b, contact.r_b);
    let vn = v_rel.dot(contact.normal);
    let mut delta_normal = contact.normal_mass * (contact.velocity_bias - vn);

    let new_normal = (contact.accum_n + delta_normal).max(0.0);
    delta_normal = new_normal - contact.accum_n;
    contact.accum_n = new_normal;

    let impulse = delta_normal * contact.normal;
    velocity_a.linear += contact.inv_mass_a * impulse;
    velocity_a.angular += contact.inv_ia * cross(contact.r_a, impulse);
    velocity_b.linear -= contact.inv_mass_b * impulse;
    velocity_b.angular -= contact.inv_ib * cross(contact.r_b, impulse);

    let v_rel = vel_at(&velocity_a, contact.r_a) - vel_at(&velocity_b, contact.r_b);
    let vt = v_rel.dot(contact.tangent);
    let mut delta_tangent = contact.tangent_mass * -vt;

    let max_friction = contact.friction * contact.accum_n;
    let new_tangent = (contact.accum_t + delta_tangent).clamp(-max_friction, max_friction);
    delta_tangent = new_tangent - contact.accum_t;
    contact.accum_t = new_tangent;

    let tangent_impulse = delta_tangent * contact.tangent;
    velocity_a.linear += contact.inv_mass_a * tangent_impulse;
    velocity_a.angular += contact.inv_ia * cross(contact.r_a, tangent_impulse);
    velocity_b.linear -= contact.inv_mass_b * tangent_impulse;
    velocity_b.angular -= contact.inv_ib * cross(contact.r_b, tangent_impulse);
}

fn update_sleep(
    dt: f32,
    bodies: &mut Query<(Entity, &mut RigidBody)>,
    velocities: &mut Query<&mut Velocity>,
) {
    let lin_sleep_tol_sq = LIN_SLEEP_TOL * LIN_SLEEP_TOL;
    for (entity, mut body) in bodies.iter_mut() {
        let Ok(mut velocity) = velocities.get_mut(entity) else {
            continue;
        };

        if body.mass <= 0.0 || body.asleep {
            continue;
        }

        if velocity.linear.length_squared() < lin_sleep_tol_sq
            && velocity.angular.abs() < ANG_SLEEP_TOL
        {
            body.sleep_timer += dt;
            if body.sleep_timer >= SLEEP_TIME {
                body.asleep = true;
                velocity.linear = Vec2::ZERO;
                velocity.angular = 0.0;
            }
        } else {
            body.sleep_timer = 0.0;
        }
    }
}

fn approach_normal_speed(
    entity_a: Entity,
    entity_b: Entity,
    contact: collision::ContactInfo,
    transforms: &Query<&mut Transform>,
    velocities: &Query<&mut Velocity>,
) -> f32 {
    let position_a = transforms
        .get(entity_a)
        .map(|transform| transform.translation.truncate())
        .unwrap_or(Vec2::ZERO);
    let position_b = transforms
        .get(entity_b)
        .map(|transform| transform.translation.truncate())
        .unwrap_or(Vec2::ZERO);
    let velocity_a = velocities
        .get(entity_a)
        .map(|velocity| vel_at(velocity, contact.contact_point - position_a))
        .unwrap_or(Vec2::ZERO);
    let velocity_b = velocities
        .get(entity_b)
        .map(|velocity| vel_at(velocity, contact.contact_point - position_b))
        .unwrap_or(Vec2::ZERO);

    (-(velocity_a - velocity_b).dot(contact.normal)).max(0.0)
}

fn vel_at(velocity: &Velocity, r: Vec2) -> Vec2 {
    velocity.linear + Vec2::new(-velocity.angular * r.y, velocity.angular * r.x)
}

fn cross(a: Vec2, b: Vec2) -> f32 {
    a.x * b.y - a.y * b.x
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn overlapping_dynamic_circles_are_separated() {
        let mut app = App::new();
        app.add_event::<CollisionEvent>()
            .init_resource::<CollisionGrid>()
            .insert_resource(Time::<Fixed>::from_seconds(1.0 / 120.0))
            .add_systems(Update, collision_system);

        let collider = Collider {
            shape: collision::Shape::Circle { radius: 10.0 },
            layer: game_layers::ASTEROID,
            mask: game_layers::ASTEROID,
            sensor: false,
        };
        let entity_a = app
            .world_mut()
            .spawn((
                Transform::from_translation(Vec3::ZERO),
                Velocity::default(),
                RigidBody {
                    mass: 1.0,
                    inertia: 1.0,
                    ..default()
                },
                Collider {
                    shape: collider.shape.clone(),
                    ..collider
                },
            ))
            .id();
        let entity_b = app
            .world_mut()
            .spawn((
                Transform::from_translation(Vec3::new(15.0, 0.0, 0.0)),
                Velocity::default(),
                RigidBody {
                    mass: 1.0,
                    inertia: 1.0,
                    ..default()
                },
                Collider {
                    shape: collision::Shape::Circle { radius: 10.0 },
                    layer: game_layers::ASTEROID,
                    mask: game_layers::ASTEROID,
                    sensor: false,
                },
            ))
            .id();

        app.update();

        let transform_a = app.world().get::<Transform>(entity_a).unwrap();
        let transform_b = app.world().get::<Transform>(entity_b).unwrap();
        let distance = transform_a
            .translation
            .truncate()
            .distance(transform_b.translation.truncate());
        assert!(distance > 15.0);
    }
}

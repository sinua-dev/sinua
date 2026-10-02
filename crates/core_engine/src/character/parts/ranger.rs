//! BUZZY's parts: a torso with a voice core in the chest, chevron ear pods
//! with listening arcs (behind the helmet, and the near one in front once the
//! head turns), a rounded-square helmet (side band, light, shade), an amber
//! fin, and the face screen with its visor.

use std::f64::consts::PI;

use crate::character::face::{self, Face};
use crate::character::geom::{self, blurred, linear, outline, pt, radial, solid, Xf};
use crate::character::kit;
use crate::character::parts::Reader;
use crate::character::recipe::{Ctx, Space};
use crate::character::turn;
use crate::primitives::Fill;

/// The torso: a rounded shoulder shape, lit top to bottom.
pub fn torso(r: &mut Reader, space: Space, ctx: &Ctx, out: &mut Vec<Fill>) {
    let [l, m, d] = r.cols::<3>();
    let lw = r.n() * ctx.lw;
    let line = ctx.colour(r.col());
    let torso = geom::join(&[
        geom::cubic(
            (62.0, 186.0),
            (62.0, 158.0),
            (74.0, 146.0),
            (100.0, 146.0),
            16,
        ),
        geom::cubic(
            (100.0, 146.0),
            (126.0, 146.0),
            (138.0, 158.0),
            (138.0, 186.0),
            16,
        ),
    ]);
    out.push(ctx.place(
        linear(
            torso.clone(),
            (0.0, 146.0),
            (0.0, 186.0),
            &[
                (0.0, ctx.colour(l)),
                (0.35, ctx.colour(m)),
                (1.0, ctx.colour(d)),
            ],
        ),
        space,
        None,
    ));
    out.push(ctx.place(outline(&torso, lw, line, 1.0), space, None));
}

/// The chest's voice core: a dark disc, a ring and bars with the voice; it
/// follows the head turn at 40 %.
pub fn chest_core(r: &mut Reader, space: Space, ctx: &Ctx, out: &mut Vec<Fill>) {
    let (x, y) = r.p2();
    let rad = r.n();
    let bars = r.list();
    let [disc, ring, bar_c] = r.cols::<3>();
    let pose = &ctx.pose;
    let core_dx = 30.0 * (0.4 * ctx.tn.yaw).sin();
    let core_on = 0.3 + 0.6 * pose.drive;
    let core = geom::ellipse(x + core_dx, y, rad, rad, 0.0, 32);
    out.push(ctx.place(solid(core.clone(), ctx.colour(disc), 1.0), space, None));
    out.push(ctx.place(
        outline(&core, 2.5 * ctx.lw, ctx.colour(ring), 0.5 + 0.5 * core_on),
        space,
        None,
    ));
    if !ctx.tier.small {
        for (i, b) in bars.iter().enumerate() {
            let idle = 0.1 + 0.05 * (ctx.t * 2.0 + i as f64).sin();
            let h = 2.0 + b * pose.drive.max(idle) * 12.0;
            let bar = geom::round_rect(
                x - 8.0 + i as f64 * 4.0 - 1.2 + core_dx,
                y - h / 2.0,
                2.4,
                h,
                1.2,
                2.0,
            );
            let bar = geom::clip_convex(&bar, &core);
            if bar.len() >= 3 {
                out.push(ctx.place(
                    solid(bar, ctx.colour(bar_c), 0.55 + 0.45 * core_on),
                    space,
                    None,
                ));
            }
        }
    }
}

/// The ear pods with their chevrons and the listening arcs. Turned, the pods
/// swing round the head: the far one narrows, the near one widens. `front`:
/// the near pod's copy in front of the helmet, fading in as it turns so
/// nothing jumps at the switch.
pub fn ear_pods(r: &mut Reader, space: Space, ctx: &Ctx, out: &mut Vec<Fill>) {
    let x0 = r.n();
    let reach = r.n();
    let front_copy = r.b();
    let [pod, line, chev, arcs] = r.cols::<4>();
    let tn = &ctx.tn;
    let (sy, cy) = (tn.yaw.sin(), tn.yaw.cos());
    let pod_at = |s: f64| {
        let w = 16.0 * (1.0 - 0.5 * s * sy);
        let x = x0 + s * reach * cy;
        (
            x - (x0 + s * reach),
            geom::round_rect(x - w / 2.0, 76.0, w, 34.0, 7.0, 3.0),
        )
    };
    let (pod_c, line, chev_c, arc_c) = (
        ctx.colour(pod),
        ctx.colour(line),
        ctx.colour(chev),
        ctx.colour(arcs),
    );
    let lw = ctx.lw;
    let on = !ctx.tier.small && ctx.accessories;
    let chevron = |s: f64, dx: f64| {
        [
            pt(x0 + s * 64.0 + dx, 87.0),
            pt(x0 + s * 60.0 + dx, 93.0),
            pt(x0 + s * 64.0 + dx, 99.0),
        ]
    };
    if front_copy {
        let near = tn.near_side();
        let front = tn.fade_in();
        if front > 0.0 {
            let (dx, pod) = pod_at(near);
            out.push(ctx.place(solid(pod.clone(), pod_c, front), space, None));
            out.push(ctx.place(outline(&pod, 3.0 * lw, line, front), space, None));
            if on {
                out.push(ctx.place(
                    solid(geom::stroke(&chevron(near, dx), 3.0), chev_c, front),
                    space,
                    None,
                ));
            }
        }
        return;
    }
    let pose = &ctx.pose;
    for s in [-1.0, 1.0] {
        let (dx, pod) = pod_at(s);
        out.push(ctx.place(solid(pod.clone(), pod_c, 1.0), space, None));
        out.push(ctx.place(outline(&pod, 3.0 * lw, line, 1.0), space, None));
        if on {
            out.push(ctx.place(
                solid(geom::stroke(&chevron(s, dx), 3.0), chev_c, 1.0),
                space,
                None,
            ));
            // Turned, the far pod's arcs go behind the helmet with it.
            let far = if s == -tn.near_side() {
                1.0 - tn.fade_in()
            } else {
                1.0
            };
            for k in 0..2 {
                let a = (pose.ears * (pose.drive * 1.6 - k as f64 * 0.5)).min(1.0) * far;
                if a <= 0.02 {
                    continue;
                }
                let (a0, a1) = if s > 0.0 {
                    (-0.7, 0.7)
                } else {
                    (PI - 0.7, PI + 0.7)
                };
                let arc = geom::arc(x0 + s * 70.0 + dx, 93.0, 9.0 + k as f64 * 7.0, a0, a1, 12);
                let stroke = geom::stroke(&arc, 2.6);
                out.push(ctx.place(blurred(stroke.clone(), arc_c, 0.5 * a, 2.0), space, None));
                out.push(ctx.place(solid(stroke, arc_c, a), space, None));
            }
        }
    }
}

/// The helmet: a rounded square lit from the top left and shaded bottom right;
/// turned, its back edge shows as a side band and the light stays put.
pub fn helmet(r: &mut Reader, space: Space, ctx: &Ctx, out: &mut Vec<Fill>) {
    let [l, m, d] = r.cols::<3>();
    let line = ctx.colour(r.col());
    let tn = &ctx.tn;
    let sy = tn.yaw.sin();
    let (shell_l, shell, shell_d) = (ctx.colour(l), ctx.colour(m), ctx.colour(d));
    let lw = ctx.lw;
    let head = geom::superellipse(100.0, 92.0, 56.0, 52.0, 4.2, 96);
    if !tn.is_zero() {
        if let Some((band, rim)) = turn::Turn::side_band(&head, (-16.0 * sy, 16.0 * tn.pitch.sin()))
        {
            out.push(ctx.place(solid(band, shell_d, 1.0), space, None));
            out.push(ctx.place(solid(geom::stroke(&rim, 3.5 * lw), line, 1.0), space, None));
        }
    }
    let (lx, ly) = (-14.0 * sy, 10.0 * tn.pitch.sin());
    out.push(ctx.place(
        radial(
            head.clone(),
            (78.0 + lx, 60.0 + ly),
            78.0,
            &[(0.0, shell_l), (0.55, shell), (1.0, shell_d)],
        ),
        space,
        None,
    ));
    let shade = geom::clip_convex(
        &geom::ellipse(126.0 + lx, 138.0 + ly, 60.0, 34.0, -0.4, 48),
        &head,
    );
    if shade.len() >= 3 {
        out.push(ctx.place(solid(shade, shell_d, 0.35), space, None));
    }
    out.push(ctx.place(outline(&head, 3.5 * lw, line, 1.0), space, None));
}

/// The fin on top of the helmet (64 and 32 px, with accessories).
pub fn fin(r: &mut Reader, space: Space, ctx: &Ctx, out: &mut Vec<Fill>) {
    if !(!ctx.tier.small && ctx.accessories) {
        return;
    }
    let c = ctx.colour(r.col());
    let line = ctx.colour(r.col());
    let crest = geom::join(&[
        geom::quad((86.0, 44.0), (100.0, 22.0), (114.0, 44.0), 12),
        geom::quad((114.0, 44.0), (100.0, 38.0), (86.0, 44.0), 8),
    ]);
    let crest = if ctx.tn.is_zero() {
        crest
    } else {
        Xf::translate(22.0 * ctx.tn.yaw.sin(), 0.0).map(&crest)
    };
    out.push(ctx.place(solid(crest.clone(), c, 1.0), space, None));
    out.push(ctx.place(outline(&crest, 2.5, line, 1.0), space, None));
}

/// The face screen: the eyes and the mouth on it (clipped to it), its rim, and
/// the glass visor with a slowly drifting reflection (64 px only). All of it
/// rides the face surface when the head turns.
pub fn face_screen(r: &mut Reader, space: Space, ctx: &Ctx, out: &mut Vec<Fill>) {
    let [screen, ink, line, glass, glass_edge, glint_c] = r.cols::<6>();
    let surface = r.surf();
    let (style, iris, sclera) = ctx.eye_look(r);
    let small = ctx.tier.small;
    let pose = &ctx.pose;
    let mut fills = Vec::new();
    let scr = geom::round_rect(58.0, 62.0, 84.0, 62.0, 22.0, 2.0);
    fills.push(solid(scr.clone(), ctx.colour(screen), 1.0));
    let f = Face {
        cx: 100.0,
        cy: 90.0,
        scale: if small { 1.08 } else { 0.92 },
        ink: kit::effect_ink(pose, ctx.colour(ink)),
        glow: if small { 2.0 } else { 4.0 },
        clip: Some(&scr),
        style,
        iris,
        sclera,
        small,
    };
    fills.extend(face::eye_fills(&f, &pose.eyes));
    fills.extend(face::mouth_fills(
        &f,
        &pose.eyes,
        pose.mouth,
        if small { 11.0 } else { 18.0 },
    ));
    fills.push(outline(&scr, 2.5 * ctx.lw, ctx.colour(line), 1.0));
    if ctx.tier.full {
        let visor = geom::ellipse(100.0, 88.0, 50.0, 44.0, 0.0, 64);
        fills.push(solid(visor.clone(), ctx.colour(glass), 0.16));
        fills.push(outline(&visor, 1.6, ctx.colour(glass_edge), 0.55));
        let sw = 0.15 + 0.1 * (ctx.t * 0.5).sin();
        let glint = geom::arc(
            100.0,
            88.0,
            40.0,
            PI * (1.12 + sw * 0.5),
            PI * (1.32 + sw * 0.5),
            12,
        );
        let glint = geom::clip_convex(&geom::stroke(&glint, 4.0), &visor);
        if glint.len() >= 3 {
            fills.push(solid(glint, ctx.colour(glint_c), 0.55));
        }
    }
    out.extend(fills.into_iter().map(|x| ctx.place(x, space, surface)));
}

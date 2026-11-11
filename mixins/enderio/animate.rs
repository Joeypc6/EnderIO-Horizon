fn enderio_animate(engine: &mut ParticleEngine, ctx: &AnimateContext<'_>, player: [i32; 3]) {
    enderio_particles::animate(engine, ctx, player);
}

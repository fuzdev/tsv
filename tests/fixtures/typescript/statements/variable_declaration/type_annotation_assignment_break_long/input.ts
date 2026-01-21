// 100 chars total - stays on one line
export const Aaaaaaaaaaaaaaaaaaaaaaaaaaaa: Record<Bbbbbbbbbbbbbbbbbbbbbbbbbbbbb, Cccccccccc> = init;

// 101 chars total - breaks after `=`, LHS type annotation stays together
export const Aaaaaaaaaaaaaaaaaaaaaaaaaaaaa: Record<Bbbbbbbbbbbbbbbbbbbbbbbbbbbbb, Cccccccccc> =
	init;

// RHS with `as` that fits on one line - breaks after `=`, both type annotations stay together
export const Aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa1: Record<Bbbbbbbbbbbbbbbb, Cccccccccc> =
	Eeeeee.fffffffffff(ddddddddddddddddddddddddddddddddddddddddd) as Record<Bbbbbbbbbbbbbbbb, Cccccc>;

// RHS with `as` that breaks - breaks after `=`, LHS type stays together, RHS type breaks internally
export const Aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa2: Record<Bbbbbbbbbbbbbbbb, Cccccccccc> =
	Eeeeee.fffffffffff(ddddddddddddddddddddddddddddddddddddddddd) as Record<
		Bbbbbbbbbbbbbbbb,
		Ccccccc
	>;

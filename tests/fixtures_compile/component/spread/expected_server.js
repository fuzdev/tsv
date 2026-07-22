import * as $ from 'svelte/internal/server';
export default function Input($$renderer, $$props) {
	let { r } = $$props;
	Foo($$renderer, $.spread_props([{ a: 1 }, r, { b: 2 }]));
}

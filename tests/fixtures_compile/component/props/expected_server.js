import * as $ from 'svelte/internal/server';
export default function Input($$renderer, $$props) {
	let { x, value } = $$props;
	Foo($$renderer, { a: 's', b: x, value, disabled: true });
}

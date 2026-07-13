import * as $ from 'svelte/internal/server';
export default function Input($$renderer, $$props) {
	let { y } = $$props;
	Foo($$renderer, { a: `x ${$.stringify(y)} z` });
}

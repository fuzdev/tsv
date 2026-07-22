import * as $ from 'svelte/internal/server';
import Foo from './Foo.svelte';
export default function Input($$renderer) {
	Foo($$renderer, {});
}

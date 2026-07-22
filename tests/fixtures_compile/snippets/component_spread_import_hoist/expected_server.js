import * as $ from 'svelte/internal/server';
import Foo from './Foo.svelte';
import { cfg } from './cfg.js';
function s($$renderer) {
	Foo($$renderer, $.spread_props([cfg]));
}
export default function Input($$renderer) {
	s($$renderer);
}

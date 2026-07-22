import * as $ from 'svelte/internal/server';
import { helper } from './helper.js';
const SHARED = 5;
export const version = '1.0';
function util() {
	return SHARED;
}
export default function Input($$renderer) {
	$$renderer.push(`<p>${$.escape(helper)}</p>`);
}

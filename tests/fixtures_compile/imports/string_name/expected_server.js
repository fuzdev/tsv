import * as $ from 'svelte/internal/server';
import { 'a-b' as loc } from './y.js';
export default function Input($$renderer) {
	$$renderer.push(`<!---->${$.escape(loc)}`);
}

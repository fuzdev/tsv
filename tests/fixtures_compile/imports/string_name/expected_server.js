import * as $ from 'svelte/internal/server';
import { loc } from './y.js';
export default function Input($$renderer) {
	$$renderer.push(`<!---->${$.escape(loc)}`);
}

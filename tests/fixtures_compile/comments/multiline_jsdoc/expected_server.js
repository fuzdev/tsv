import * as $ from 'svelte/internal/server';
function helper() {}
/**
 * Shared across instances.
 */
export const shared = 1;
export default function Input($$renderer, $$props) {
	$$renderer.component(($$renderer) => {
		/**
		 * The count shown below.
		 * @type {number}
		 */
		let count = shared;
		class Tally {
			/**
			 * Bump it.
			 */
			bump() {
				/*
				 * inner note
				 */
				count += 1;
			}
		}
		const tally = new Tally();
		helper();
		$$renderer.push(`<button>${$.escape(count)}</button>`);
	});
}

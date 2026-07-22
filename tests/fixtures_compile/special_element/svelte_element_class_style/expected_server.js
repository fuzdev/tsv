import * as $ from 'svelte/internal/server';
export default function Input($$renderer) {
	$.element(
		$$renderer,
		tag,
		() => {
			$$renderer.push(
				`${$.attr_class('', void 0, { active: on })}${$.attr_style('', { color: c })}`
			);
		},
		() => {
			$$renderer.push(`hi`);
		}
	);
}

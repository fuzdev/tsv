<!-- Short expression - no wrap -->
<div onclick={() => fn()}></div>

<!-- Short expression attr - no wrap -->
<div data-x={a + b}></div>

<!-- Block body - wraps due to hardlines (not width) -->
<div
	onclick={() => {
		fn();
	}}
></div>

<!-- Long expression without hardlines - wraps due to width -->
<div
	data-computed={fn000000000000000000(arg00000000, arg11111111, arg2222222222, arg333333333)}
></div>

<!-- Multiple attrs with one having hardlines - all wrap -->
<div
	class="class1"
	onclick={() => {
		fn();
	}}
	id="id1"
></div>

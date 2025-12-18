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
	data-computed={veryLongFunctionName(argumentOne, argumentTwo, argumentThree, argumentFour)}
></div>

<!-- Multiple attrs with one having hardlines - all wrap -->
<div
	class="container"
	onclick={() => {
		handleClick();
	}}
	id="main"
></div>

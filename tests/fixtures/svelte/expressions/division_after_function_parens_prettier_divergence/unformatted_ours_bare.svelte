<script lang="ts">
	// a function or class expression as the left operand of `/` keeps its pair
	x = (function () {}) / 2;
	x = c ? y : (function () {}) / 2;
	x = a + (async function () {}) / 2;
	x = a + function () {} / 2;
	x = [c ? y : (class {}) / 2];
	(function () {}) / 2;
	(class {}) / 2 / 3;

	// so does one the left operand ends on: under a prefix operator, as the right operand of an
	// operator that binds as tightly, under an angle-bracket assertion
	x = !function () {} / 2;
	x = typeof (async function () {}) / 2;
	x = -class {} / 2;
	x = a / function () {} / 2;
	x = a / (async function () {}) / 2 / 3;
	x = a ** class {} / 2;
	x = <any>function () {} / 2;
	x = a / !function () {} / 2;
	x =
		!(
			@dec
			class {}
		) / 2;

	// a left operand that ends on a pair of its own, or one no `/` follows, leaves it bare
	x = (a * function () {}) / 2;
	x = !function () {} * 2;

	// the pair holds a block comment its operand owns; one after the operand prints past the `)`
	(/* c */ function () {}) / 2;
	x = function () {} /* c */ / 2;

	// after `export default` the pair is the dividend's own, printed once
	export default (function () {}) / 2;
</script>

<!-- a function or class expression as the left operand of `/` keeps its pair -->
<div>{(function () {}) / 2}</div>
<div>{(class {}) / 2}</div>
<div>{(async function () {}) / 2}</div>
<div>{async function fn() {} / 2}</div>
<div>{async function* () {} / 2}</div>
<div>{(function* () {}) / 2}</div>
<div>{(function fn() {}) / 2}</div>
<div>{(class A extends B {}) / 2}</div>
<div>{(function () {}) / 2 / 3}</div>
<div>{x + function () {} / 2}</div>
<div>{c ? x : (function () {}) / 2}</div>
<div>{c ? class {} / 2 : x}</div>
<div>{fn(c ? x : function () {} / 2)}</div>
<div>{`${function () {} / 2}`}</div>
<div data-attr={(function () {}) / 2}></div>
<div {...(function () {}) / 2}></div>
{#if (function () {}) / 2}text{/if}
{#each (class {}) / 2 as item}text{/each}

<!-- so does one the left operand ends on -->
<div>{!function () {} / 2}</div>
<div>{typeof (async function () {}) / 2}</div>
<div>{a / function () {} / 2}</div>
<div>{a ** class {} / 2}</div>
<div>{<any>function () {} / 2}</div>

<!-- with no `/` after it the operand stays bare -->
<div>{2 / function () {}}</div>
<div>{function () {} * 2}</div>
<div>{!function () {}}</div>
<div>{!function () {} * 2}</div>

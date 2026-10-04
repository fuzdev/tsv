<div title={/** @type {A} */ (x) /* t1 */}>text</div>

{#if /** @type {B} */ (x) /* t2 */}yes{/if}

{@html /** @type {C} */ (x) /* t3 */}

<button onclick={() => f(/** @type {D} */ (e) /* t4 */)}>go</button>

<div title={/** @type {A} */ (x) /* t5 */ /* t6 */}>text</div>

{#if /** @type {B} */ (x) // t7
// t8
}
	yes
{/if}

{@html /** @type {C} */ (x) /* t9 */ /* t10 */}

<input bind:value={/** @type {V} */ (v) /* t11 */ /* t12 */} />

<div {.../** @type {P} */ (p) /* t13 */ /* t14 */}></div>

{#each /** @type {L} */ (xs) /* t15 */ /* t16 */ as x}{x}{/each}

{#if x}
	{@const a = /** @type {K} */ (x) /* t17 */ /* t18 */}
	{a}
{/if}

{@render /** @type {R} */ (r()) /* t19 */ /* t20 */}

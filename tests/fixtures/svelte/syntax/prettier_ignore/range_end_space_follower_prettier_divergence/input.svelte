<!-- A space after the end marker is the follower's own per-width wrap, as after any comment -->
<!-- prettier-ignore-start --><b >text1</b><!-- prettier-ignore-end --> <Comp />

<!-- prettier-ignore-start --><b >text1</b><!-- prettier-ignore-end --> {expr}

<!-- prettier-ignore-start --><b >text1</b><!-- prettier-ignore-end --> {@html expr}

<!-- prettier-ignore-start --><b >text1</b><!-- prettier-ignore-end --> {#if cond}text2{/if}

<!-- prettier-ignore-start -->text1<!-- prettier-ignore-end --> {#each items as item}{item}{/each}

<!-- prettier-ignore-start -->
<b >text1</b>
<!-- prettier-ignore-end --> <svelte:element this="p">text2</svelte:element>

<!-- prettier-ignore-start --><b >text1</b><!-- prettier-ignore-end --> <!-- c -->

<!-- prettier-ignore-start -->text1<!-- prettier-ignore-end --> <!-- prettier-ignore-start -->
<b >text2</b>
<!-- prettier-ignore-end -->

<!-- Sequence expressions in directive values require parentheses -->

<!-- Simple sequence expression -->
<div use:action={(a,b,c)}></div>

<!-- Longer sequence expression -->
<div use:action={(expr1,expr2,expr3,expr4,expr5)}></div>

<!-- Sequence in other directives -->
<div on:click={(first(),second(),third())}></div>
<div transition:fade={(prep(),getConfig())}></div>

<!-- 100 cols: fits, the control-flow block hugs the inline sibling on one line -->
{#if shown}A{/if} <span>inline filler content used to reach the print width boundary abcdefgh</span>

<!-- 101 cols: exceeds print width, so the block and its inline sibling break apart -->
{#if shown}A{/if} <span>inline filler content used to reach the print width boundary abcdefghi</span>

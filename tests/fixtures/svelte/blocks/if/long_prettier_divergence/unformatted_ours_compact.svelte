<div>
<!-- 100 chars - stays inline (at boundary) -->
{#if a&&b&&c&&d&&e&&f&&g&&h&&i&&j&&k&&l&&m&&n&&o&&p&&rrrrrr}x{/if}

<!-- 101 chars - wraps with continuation indent (divergence from Prettier) -->
{#if a&&b&&c&&d&&e&&f&&g&&h&&i&&j&&k&&l&&m&&n&&o&&p&&rrrrrrr}x{/if}

<!-- 107 chars with function call - wraps (divergence from Prettier) -->
{#if a&&fn(arg00000000,arg11111111,arg2222222222,arg333333333,arg444444444,arg55555555)&&b}x{/if}

<!-- {:else if} at 101 chars - wraps (divergence from Prettier) -->
{#if a}
x
{:else if b&&c&&d&&e&&f&&g&&h&&i&&j&&k&&l&&m&&n&&o&&p&&q&&rrrrrrrr}
y
{/if}
</div>

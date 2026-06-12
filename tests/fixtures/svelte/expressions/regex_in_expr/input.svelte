<!-- Regex with } inside -->
<div onclick={() => /}/.test(str)}></div>

<!-- Regex with { inside -->
<div onclick={() => /{/.test(str)}></div>

<!-- Regex replacement -->
<div data-x={str.replace(/[{}]/g, '')}></div>

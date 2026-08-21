<!-- A foreign `<template lang="…">` body is the author's own bytes: every child rides out
	verbatim, and only the boundary newlines are normalized. -->

<!-- An element child, kept exactly as written -->
<template lang="pug">
<div   >text</div>
</template>

<!-- Text and an element in one body -->
<template lang="pug">
text1<b>text2</b>text3
</template>

<!-- A comment and an expression tag are body bytes like any other -->
<template lang="pug">
<!-- text -->
{expr}
</template>

<!-- A block child -->
<template lang="pug">
{#if cond}
text
{/if}
</template>

<!-- Nested: the body starts at column 0, the closing tag takes the element's indent -->
<div>
	<template lang="pug">
h1 text
	</template>
</div>

<!-- An empty body takes no newlines at all -->
<template lang="pug"></template>

<!-- A blank body is the two delimiter lines with nothing between them -->
<template lang="pug">

</template>

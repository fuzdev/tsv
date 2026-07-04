<!-- slot in a shadowroot template → RegularElement (suppressed), even when nested -->
<template shadowrootmode="open">
	<slot></slot>
	<div><slot></slot></div>
</template>

<!-- plain template → SlotElement -->
<template>
	<slot></slot>
</template>

<!-- top-level slot → SlotElement -->
<slot></slot>

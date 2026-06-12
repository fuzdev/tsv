<style>
/* 100 chars exactly - stays inline (at boundary) */
.class1 > .class2:not(.class3):hover > .class4 + .class5 ~ .class6[data-attr1][data-attr2abcdef] { color: red; }

/* 101 chars - wraps per combinator (exceeds print_width) */
.class1 > .class2:not(.class3):hover > .class4 + .class5 ~ .class6[data-attr1][data-attr2abcdefg] { color: red; }
</style>

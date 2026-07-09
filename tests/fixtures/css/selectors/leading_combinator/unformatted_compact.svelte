<style>
/* leading child combinator (top-level, no parent rule) */
>.class1{color:red;}

/* leading next-sibling combinator */
+.class2{color:red;}

/* leading subsequent-sibling combinator */
~.class3{color:red;}

/* leading combinator continues into a full complex selector */
>.class4>.class5{color:red;}

/* leading combinator on the first item of a selector list */
>.class6,.class7{color:red;}

/* leading combinator inside :not() (complex-selector grammar) */
:not(>.class8){color:red;}

/* leading combinator inside an at-rule body */
@media screen{>.class9{color:red;}}
</style>

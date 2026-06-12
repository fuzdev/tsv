<style>
.class1{padding:10px;

/* child combinator */
>.class2{margin:5px;}

/* next-sibling combinator */
+.class3{margin-top:20px;}

/* subsequent-sibling combinator */
~.class4{color:gray;}

/* leading combinator in selector list */
>.class5>.class6 .class7,.class8,.class9 ::selection{color:blue;}

/* combinator with multiple compound selectors */
>.class10.class11{color:red;}}
</style>

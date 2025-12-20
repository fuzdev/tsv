<style>
/* Long nested selector list in :where() - wraps when exceeding print width */
div:where(  .class1  ,  .class2  ,  .class3  ,  .class4  ,  .class5  ,  .class6  ,  .class7  ,  .class8  ,  .class9  ,  .class10  ) {
color:   red;
}

/* Long nested selector list in :is() - stays inline (under 100 chars) */
span:is(  .a  ,  .b  ,  .c  ,  .d  ,  .e  ,  .f  ,  .g  ,  .h  ,  .i  ,  .j  ) {
color:   blue;
}
</style>

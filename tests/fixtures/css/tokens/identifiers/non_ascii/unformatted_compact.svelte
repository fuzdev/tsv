<style>
/* single non-ASCII symbol in an id selector */
#♥{color:red;}

/* multiple non-ASCII code points exercise identifier continuation */
#☺☃{color:red;}

/* astral-plane code point (a surrogate pair in UTF-16) */
#💩{color:red;}

/* non-ASCII code point in a class selector */
.♥bar{color:red;}
</style>

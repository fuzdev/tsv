<script>
// A sole destructured parameter hugs the parens when its default is an empty object or
// array, or an identifier: `({` opens on the signature line and the pattern breaks inside
function f1({aaaaaaaaa1,aaaaaaaaa2,aaaaaaaaa3,aaaaaaaaa4,aaaaaaaaa5,aaaaaaaaa6,aaaaaaaaa7}={}){return 1;}function f2({bbbbbbbbb1,bbbbbbbbb2,bbbbbbbbb3,bbbbbbbbb4,bbbbbbbbb5,bbbbbbbbb6,bbbbbbbbb7}=defaults){return 1;}

// A non-empty default: at 100 the whole signature still fits
function f3({ccccccccc1,ccccccccc2,ccccccccc3,ccccccccc4,ccccccccc5,ccccccc6}={x:1}){return 1;}

// at 101 the parameter is not hugged: the list expands and the pattern stays flat
function f4({ddddddddd1,ddddddddd2,ddddddddd3,ddddddddd4,ddddddddd5,dddddddd6}={x:1}){return 1;}

// An array pattern with a non-empty default expands the same way
function f5([eeeeeeeee1,eeeeeeeee2,eeeeeeeee3,eeeeeeeee4,eeeeeeeee5,eeeeeeeee6,eeeeeeeee7]=[1]){return 1;}

// and so does a call default
function f6({fffffffff1,fffffffff2,fffffffff3,fffffffff4,fffffffff5,fffffffff6,fffffffff7}=make()){return 1;}
</script>

# windows line endings
{ sub("\r", "") }

# /*! header
$0 == "/*!" { header = 1; next }
$0 == "*/" && header { header = 0; print ""; next }
header && substr($0, 1, 2) == "  " { print "- " substr($0, 3); next }
header { print "**" $0 "**"; print ""; next }

# trim indentation
{ $1 = $1 }

# collect /// lines
$1 == "///" { comment = comment " " substr($0, 5); next }

# nothing collected or attribute between comment and code
comment == "" || substr($1, 1, 2) == "#[" { next }

# the code line the comment belongs to
{
    if ($NF == "{") $0 = substr($0, 1, length($0) - 2)
    print "- `" $0 "`:" comment
    comment = ""
}

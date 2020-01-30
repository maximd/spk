source = """
set shell [lindex $argv 0]
set startup_script [lindex $argv 1]
spawn $shell -f
expect {
    > {
        send "source '${startup_script}'\n"
    }
}
interact
"""
import TestInstance
import sys

from TestInstance import test_assert

def test(instance):
    test_assert("Kernel image start timed out", instance.wait_for_line("OK43e6H", timeout=10))
    test_assert("Init load timed out", instance.wait_for_line("Entering userland at 0x[0-9a-f]+ '/system/Tifflin/bin/loader' '/system/Tifflin/bin/init'", timeout=5))

    test_assert("Initial startup timed out", instance.wait_for_idle(timeout=20))
    instance.screenshot('Login')

    instance.type_string('root')
    while instance.wait_for_idle():
        pass
    instance.type_key('ret')
    test_assert("Username return press timeout", instance.wait_for_line("\[syscalls\] - USER> Window::handle_event\(ev=KeyDown\(Return\)\)", timeout=1)) # Press
    test_assert("Username return release timeout", instance.wait_for_line("\[syscalls\] - USER> Window::handle_event\(ev=KeyUp\(Return\)\)", timeout=1))
    test_assert("Username return release idle", instance.wait_for_idle())
    # TODO: Have an item in the log here
    
    instance.type_string('password')
    # - Wait until there's 1s with no action
    while instance.wait_for_idle():
        pass
    instance.type_key('ret')
    test_assert("Password return press timeout", instance.wait_for_line("\[syscalls\] - USER> Window::handle_event\(ev=KeyDown\(Return\)\)", timeout=1)) # Press
    test_assert("Shell startup timeout", instance.wait_for_line("\[syscalls\] - USER> Calling entry 0x[0-9a-f]+ for b\"/sysroot/bin/shell\"", timeout=1))
    test_assert("Shell idle timeout", instance.wait_for_idle(timeout=5))
    # TODO: Have an item in the log here

    # - Open the "System" menu (press left windows key)
    instance.screenshot('Shell')
    instance.type_key('meta_l')
    test_assert("Password return press timeout", instance.wait_for_line("\[syscalls\] - USER> Window::handle_event\(ev=KeyDown\(LeftGui\)\)", timeout=1)) # Press
    test_assert("Password return press timeout", instance.wait_for_line("\[syscalls\] - USER> Window::handle_event\(ev=KeyUp\(LeftGui\)\)", timeout=1))
    test_assert("System menu release timeout", instance.wait_for_idle(timeout=5)) # Release
    instance.screenshot('Menu')

    # - Select the top item to open the CLI
    instance.type_key('ret')
    test_assert("CLI startup return press timeout", instance.wait_for_idle())
    test_assert("CLI startup timeout", instance.wait_for_line("\[syscalls\] - USER> Calling entry 0x[0-9a-f]+ for b\"/sysroot/bin/simple_console\"", timeout=5))
    test_assert("CLI window render", instance.wait_for_line("\[gui::windows\] - L\d+: WindowGroup::redraw: \d+ 'Console'", timeout=5))
    test_assert("CLI idle timeout", instance.wait_for_idle(timeout=3))
    instance.screenshot('CLI')

    # - Run a command
    instance.type_string('ls /system')
    while instance.wait_for_idle():
        pass
    instance.type_string('/Tifflin/bin')
    while instance.wait_for_idle():
        pass
    instance.type_key('ret')
    test_assert("`ls` return press timeout", instance.wait_for_line("\[syscalls\] - USER> Window::handle_event\(ev=KeyDown\(Return\)\)", timeout=1)) # Press
    test_assert("`ls` return release timeout", instance.wait_for_line("\[syscalls\] - USER> Window::handle_event\(ev=KeyUp\(Return\)\)", timeout=1)) # Release
    test_assert("Run `ls` idle timeout", instance.wait_for_idle(timeout=5))
    instance.screenshot('ls')

    # - Quit shell
    instance.type_string('exit')
    while instance.wait_for_idle():
        pass
    instance.type_key('ret')
    test_assert("`exit` return release timeout", instance.wait_for_line("\[syscalls\] - USER> Window::handle_event\(ev=KeyUp\(Return\)\)", timeout=1)) # Release
    test_assert("`exit` reap", instance.wait_for_line("Reaping thread 0x[0-9a-f]+\(\d+ /sysroot/bin/simple_console#1\)", timeout=2))
    instance.screenshot('exit')
    # DISABLED: Idle triggers reaping
    #test_assert("`ls` idle timeout", instance.wait_for_idle(timeout=5))
    
    # - Ensure that the GUI re-renders, and that the terminal no-longer shows
    test_assert("final render", instance.wait_for_line("WindowGroup::redraw: render_order=\[\(1, \[\]\), \(4, \[\(0,20 \+ \d+x\d+\)\]\), \(5, \[\(0,0 \+ \d+x20\)\]\)\]", timeout=5))


    while instance.wait_for_idle(timeout=2):
        pass
    instance.screenshot('final')


try:
    test( TestInstance.Instance("amd64", "CLI") )
except TestInstance.TestFail as e:
    print "TEST FAILURE:",e
    sys.exit(1)

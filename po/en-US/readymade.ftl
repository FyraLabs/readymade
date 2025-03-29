prev = Previous
next = Next

parttype-root = Filesystem root ({$path})
parttype-extendedboot = Extended Boot Loader Partition ({$path})
parttype-esp = EFI System Partition ({$path})
parttype-home = User data ({$path})
parttype-var = Variable data ({$path})
parttype-other = Custom partitioning mountpoint


page-welcome = Welcome to {$distro}
page-welcome-desc = Either test {$distro} from this installer or start the installation now. You can always return to this screen by selecting "Installer" in the menu.
page-welcome-try = Try
page-welcome-install = Install

page-failure = Installation Failure
page-failure-close = Close
page-failure-bug = Report a bug

page-language = Language
page-language-search-lang = Search Language/Locale…
page-language-next = Next

page-completed = Complete
page-completed-desc = Installation complete. You may reboot now and enjoy your fresh system.
page-completed-close = Close
page-completed-reboot = Reboot

page-destination = Destination

page-installdual = Dual Boot
page-installdual-otheros = Other OS

page-confirmation = Confirmation

page-installation = Installation
page-installation-welcome-desc = Get to know your new operating system.
page-installation-help = Need help?
page-installation-help-desc = Ask in one of our chats!
page-installation-contrib = Contribute to Ultramarine
page-installation-contrib-desc = Learn how to contribute your time, money, or hardware.
page-installation-progress = Installing base system...

page-installcustom = Custom Installation
page-installcustom-title = Partitions and Mountpoints
page-installcustom-desc = {$num} definition(s)
page-installcustom-tool = Open partitioning tool
page-installcustom-add = Add a new definition/row

page-installationtype = Installation Type
page-installationtype-entire = Entire Disk
page-installationtype-tpm = Enable TPM
page-installationtype-encrypt = Enable disk encryption
page-installationtype-chromebook = Chromebook
page-installationtype-dual = Dual Boot
page-installationtype-custom = Custom

dialog-installtype-encrypt = Disk Encryption
dialog-installtype-encrypt-desc = Please set the disk encryption password.
    If you lose the password, your data will not be recoverable.
dialog-installtype-password = Password
dialog-installtype-repeat = Repeat Password
dialog-installtype-cancel = Cancel
dialog-installtype-confirm = Confirm

installtype-edit-mp = Edit mountpoint
installtype-rm-mp = Remove mountpoint

dialog-mp-part = Partition
dialog-mp-at = Mount at
dialog-mp-opts = Mount options

installtype-parttool = Select your partitioning tool

stage-extracting = Extracting files
stage-copying = Copying files
stage-mkpart = Creating partitions and copying files
stage-initramfs = Regenerating initramfs
stage-grub = Generating system grub defaults
stage-grub1 = Generating stage 1 grub.cfg in ESP...
stage-grub2 = Generating stage 2 grub.cfg in /boot/grub2/grub.cfg...
stage-biosgrub = Installing BIOS Grub2
stage-kernel = Reinstalling kernels
stage-selinux = Setting SELinux labels

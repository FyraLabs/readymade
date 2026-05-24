prev = წინა
next = შემდეგი
unknown-os = უცნობი ოს
parttype-root = ფაილური სისტემის ძირითადი საქაღლდე ({ $path })
parttype-extendedboot = გაფართოებული ჩამტვირთავის დანაყოფი ({ $path })
parttype-esp = EFI სისტემური დანაყოფი ({ $path })
parttype-home = მომხმარებლის მონაცემები ({ $path })
parttype-var = ცვლადი მონაცემები ({ $path })
parttype-other = ხელიტ შექმნილი დანაყოფის მიმაგრების წერტილი
page-welcome = მოგესალმებათ { $distro }
page-welcome-desc = შეგიძლიათ, სცადოთ { $distro }, ან დაიწყოთ დაყენება.
page-welcome-try = ცდა
page-welcome-install = დაყენება
page-failure = დაყენების შეცდომა
page-failure-close = დახურვა
page-failure-bug = ანგარიში შეცდომაზე
page-language = ენა
page-language-next = შემდეგი
page-completed = დასრულებულია
page-completed-close = დახურვა
page-completed-reboot = გადატვირთვა
page-destination = სამიზნე
page-confirmation = დადასტურება
page-installation = მიმდინარეობს დაყენება
page-installationtype-chromebook = Chromebook
page-installationtype-custom = ხელით
dialog-installtype-password = პაროლი
dialog-installtype-cancel = გაუქმება
dialog-installtype-confirm = დადასტურება
dialog-mp-part = დანაყოფი
page-language-search-lang = ენის/ლოკალის ძებნა…
page-destination-scanning = დისკების სკანირება
page-installdual = ორმაგი ჩატვირთვა
page-installdual-otheros = სხვა ოს
page-installation-help = დახმარება გჭირდებათ?
page-installcustom = მორგებული დაყენება
page-installationtype = დაყენების ტიპი
page-installationtype-entire = მთელი დისკი
page-installationtype-tpm = TPM-ის ჩართვა
page-installationtype-dual = ორმაგი ჩატვირთვა
dialog-installtype-encrypt = დისკის დაშიფვრა
dialog-installtype-repeat = გაიმეორეთ პაროლი
installtype-edit-mp = მიმაგრების წერტილის ჩასწორება
installtype-rm-mp = მიმაგრების წერტილის წაშლა
dialog-mp-at = მიმაგრების წერტილი
dialog-mp-opts = მიმაგრების პარამეტრები
stage-extracting = ფაილების ამოღება
stage-copying = ფაილების კოპირება
stage-initramfs = initramfs-ის რეგენერაცია
stage-kernel = ბირთვების თავიდან დაყენება
page-destination-wait = os-prober-ის მოლოდინი…
page-destination-no-disk = დისკები აღმოჩენილი არაა
dialog-confirm-warn-efipartfound-title = აღმოჩენილია EFI დანაყოფი
page-installation-progress = საბაზისო სისტემის დაყენება...
page-installcustom-title = დანაყოფების და მიმაგრების წერტილები
page-installcustom-tool = დაყოფის პროგრამის გახსნა
page-installationtype-encrypt = დისკის დაშიფვრის ჩართვა
stage-biosgrub = BIOS Grub2-ის დაყენება
stage-selinux = SELinux-ის ჭდეების დაყენება
page-installcustom-add = ახალი აღწერის/მწკრივის დამატება
installtype-parttool = აირჩიეთ თქვენი დაყოფის პროგრამა
stage-grub = grub-ის ნაგულისხმევების რეგენერაცია
page-installation-contrib = შეწირვა დისტრიბუტივისთვის { $distro }
stage-mkpart = დანაყოფების შექმნა და ფაილების კოპირება
page-installation-help-desc = იკითხეთ ჩვენს ერთ-ერთ ჩატში!
stage-grub1 = grub.cfg-ის პირველი დონის გენერაცია ESP-ში...
stage-grub2 = მეორე დონის grub.cfg-ის გენერაცია /boot/grub2/grub.cfg-ში...
page-destination-no-disk-desc = დაყენებისთვის შესაფერისი დისკები აღმოჩენილი არაა.
page-installation-welcome-desc = გაიგეთ მეტი თქვენი ახალი ოპერაციული სისტემის შესახებ.
page-installation-contrib-desc = გაიგეთ, როგორ შემოგვწიროთ თქვენი დრო, თანხები, ან აპარატურა.
err-no-bios = /sys/firmware/efi აღმოჩენილი არაა და დისტრიბუტივს BIOS-ის მხარდაჭერა გამორთული აქვს.
page-completed-desc = დაყენება დასრულდა. შეგიძლიათ, გადატვირთოთ კომპიუტერი და ისიამოვნოთ ახალი სისტემით.
page-installcustom-desc =
    { $num } { $num ->
        [one] აღწერა
       *[other] აღწერა
    }
page-confirmation-problem-device-mounted = { $dev } მიმაგრებულია საქაღალდეზე { $mountpoint }. გასაგრძელებლად მოხსენით ის.
dialog-installtype-encrypt-desc =
    დააყენეთ დისკის დაშიფვრის პაროლი.
    თუ პაროლს დაკარგავთ, მონაცემებს ვეღარ აღადგენთ.
page-confirmation-problem-devblkopen =
    ბლოკური მოწყობილობა <tt>{ $dev }</tt> გამოიყენება შემდეგი პროცესების მიერ:
    <tt>{ $pids }</tt>
    სანამ დაყენების პროგრამა მუშაობას გააგრძელებთ, ეს პროცესები უნდა დახუროთ.
dialog-confirm-warn-efipartfound-desc =
    თუ დაყენება სხვა სისტემის გვერდზე ხდება, დარწმუნდით, რომ სამიზნე დისკზე მისი EFI დანაყოფი არ არსებობს.
    არჩეული სამიზნე დისკი შეიცავს EFI დანაყოფს, რომელიც წაიშლება და გადაფორმატდება დაყენების მიმდინარეობისას, რაც მასში რეგისტრირებულ სისტემებს ჩაუტვირთავს გახდის. ეს ქმედება შეუქცევადია.

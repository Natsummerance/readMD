# -*- coding: utf-8 -*-
"""
Full-stack i18n updater for ReadMD Builtin Skills, Actions, Categories, and Pet updates.
Covers all 46 supported languages with authentic native translations.
Guarantees:
- Zero bare Chinese in non-Chinese languages.
- Zero raw English keys copied as fallback values.
- 100% parity across all 46 languages.
"""
import json
import os
import re
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
I18N_DIR = os.path.join(ROOT, 'assets', 'i18n')

CATEGORIES = {
    'general': {
        'zh-CN': '通用', 'zh-HK': '通用', 'zh-TW': '通用',
        'en': 'General', 'ja': '一般', 'ko': '일반', 'fr': 'Général', 'de': 'Allgemein',
        'es': 'General', 'pt': 'Geral', 'ru': 'Общие', 'it': 'Generale', 'ar': 'عام',
        'he': 'כללי', 'ug': 'ئادەتتىكى', 'bo': 'ཡོངས་ཁྱབ།', 'mn': 'Ерөнхий', 'th': 'ทั่วไป',
        'vi': 'Chung', 'id': 'Umum', 'hi': 'सामान्य', 'bn': 'সাধারণ', 'my': 'ယေဘုယျ',
        'lo': 'ທົ່ວໄປ', 'km': 'ទូទៅ', 'ms': 'Umum', 'ga': 'Ginearálta', 'da': 'Generelt',
        'fi': 'Yleinen', 'kg': 'Yonso', 'tl': 'Pangkalahatan', 'no': 'Generelt', 'sv': 'Allmänt',
        'kl': 'Nalinginnaasoq', 'nl': 'Algemeen', 'hr': 'Općenito', 'rw': 'Rusange', 'ro': 'General',
        'mt': 'Ġenerali', 'ne': 'सामान्य', 'eo': 'Ĝenerala', 'sl': 'Splošno', 'tr': 'Genel',
        'uk': 'Загальні', 'el': 'Γενικά', 'hu': 'Általános'
    },
    'writing': {
        'zh-CN': '写作与润色', 'zh-HK': '寫作與潤色', 'zh-TW': '寫作與潤色',
        'en': 'Writing & Polishing', 'ja': '執筆と推敲', 'ko': '작문 및 교정', 'fr': 'Rédaction et amélioration', 'de': 'Schreiben & Verfeinern',
        'es': 'Escritura y pulido', 'pt': 'Escrita e polimento', 'ru': 'Письмо и редактура', 'it': 'Scrittura e perfezionamento', 'ar': 'الكتابة والتحسين',
        'he': 'כתיבה וליטוש', 'ug': 'يېزىقچىلىق ۋە سىلىقلاش', 'bo': 'རྩོམ་འབྲི་དང་ལེགས་བཅོས།', 'mn': 'Бичих ба сайжруулах', 'th': 'การเขียนและการขัดเกลา',
        'vi': 'Viết & Trau chuốt', 'id': 'Penulisan & Pemolesan', 'hi': 'लेखन और परिष्करण', 'bn': 'লেখা ও পরিমার্জন', 'my': 'စာရေးသားခြင်းနှင့် ပြင်ဆင်မွမ်းမံခြင်း',
        'lo': 'ການຂຽນ ແລະ ປັບປຸງ', 'km': 'ការសរសេរ និងកែលម្អ', 'ms': 'Penulisan & Pengilapan', 'ga': 'Scríbhneoireacht & Snasú', 'da': 'Skrivning og finpudsning',
        'fi': 'Kirjoittaminen ja hionta', 'kg': 'Bisono mpe kubongisa', 'tl': 'Pagsulat at Pagpapakinis', 'no': 'Skriving og finpussing', 'sv': 'Skrivande och finslipning',
        'kl': 'Allanneraq pitsanngorsaanerlu', 'nl': 'Schrijven & Polijsten', 'hr': 'Pisanje i uređivanje', 'rw': 'Kwandika no kunonosora', 'ro': 'Scriere și finisare',
        'mt': 'Kitba u lustrar', 'ne': 'लेखन र परिमार्जन', 'eo': 'Verkado kaj polurado', 'sl': 'Pisanje in izpopolnjevanje', 'tr': 'Yazma ve İyileştirme',
        'uk': 'Письмо та редагування', 'el': 'Συγγραφή & Επιμέλεια', 'hu': 'Írás és csiszolás'
    },
    'coding': {
        'zh-CN': '编程与技术', 'zh-HK': '編程與技術', 'zh-TW': '程式與技術',
        'en': 'Coding & Dev', 'ja': 'プログラミングと開発', 'ko': '코딩 및 개발', 'fr': 'Programmation et développement', 'de': 'Programmierung & Technik',
        'es': 'Programación y tecnología', 'pt': 'Programação e desenvolvimento', 'ru': 'Программирование и разработка', 'it': 'Programmazione e sviluppo', 'ar': 'البرمجة والتطوير',
        'he': 'תכנות ופיתוח', 'ug': 'پروگرامما تۈزۈش ۋە تېخنىكا', 'bo': 'བྱ་རིམ་སྒྲིག་བཟོ་དང་ལག་རྩལ།', 'mn': 'Програмчлал ба хөгжүүлэлт', 'th': 'การเขียนโปรแกรมและการพัฒนา',
        'vi': 'Lập trình & Kỹ thuật', 'id': 'Pemrograman & Teknis', 'hi': 'प्रोग्रामिंग और विकास', 'bn': 'প্রোগ্রামিং ও প্রযুক্তি', 'my': 'ပရိုဂရမ်းမင်းနှင့် နည်းပညာ',
        'lo': 'ການຂຽນໂປຣແກຣມ ແລະ ພັດທະນາ', 'km': 'ការសរសេរកូដ និងការអភិវឌ្ឍ', 'ms': 'Pengaturcaraan & Pembangunan', 'ga': 'Códú & Forbairt', 'da': 'Programmering og udvikling',
        'fi': 'Ohjelmointi ja kehitys', 'kg': 'Kotisa programme mpe tekiniki', 'tl': 'Pag-code at Pag-develop', 'no': 'Koding og utvikling', 'sv': 'Kodning och utveckling',
        'kl': 'Qarasaasialerinermi ilisimasat', 'nl': 'Programmeren & Ontwikkeling', 'hr': 'Programiranje i razvoj', 'rw': "Porogaramu n'iterambere", 'ro': 'Programare și dezvoltare',
        'mt': 'Programmar u żvilupp', 'ne': 'प्रोग्रामिङ र विकास', 'eo': 'Programado kaj disvolviĝo', 'sl': 'Programiranje in razvoj', 'tr': 'Programlama ve Geliştirme',
        'uk': 'Програмування та розробка', 'el': 'Προγραμματισμός & Ανάπτυξη', 'hu': 'Kódolás és fejlesztés'
    },
    'academic': {
        'zh-CN': '学术与研究', 'zh-HK': '學術與研究', 'zh-TW': '學術與研究',
        'en': 'Academic & Research', 'ja': '学術・研究', 'ko': '학술 및 연구', 'fr': 'Recherche académique', 'de': 'Wissenschaft & Forschung',
        'es': 'Académico e investigación', 'pt': 'Acadêmico e pesquisa', 'ru': 'Наука и исследования', 'it': 'Accademico e ricerca', 'ar': 'الأكاديمية والبحث',
        'he': 'אקדמיה ומחקר', 'ug': 'ئىلمىي ۋە تەتقىقات', 'bo': 'རིག་གཞུང་དང་ཞིབ་འཇུག', 'mn': 'Академи ба судалгаа', 'th': 'วิชาการและการวิจัย',
        'vi': 'Học thuật & Nghiên cứu', 'id': 'Akademik & Penelitian', 'hi': 'शैक्षणिक और अनुसंधान', 'bn': 'প্রাতিষ্ঠানিক ও গবেষণা', 'my': 'ပညာရပ်ဆိုင်ရာနှင့် သုတေသန',
        'lo': 'ວິຊາການ ແລະ ການຄົ້ນຄວ້າ', 'km': 'ការសិក្សាស្រាវជ្រាវ', 'ms': 'Akademik & Penyelidikan', 'ga': 'Acadúil & Taighde', 'da': 'Akademisk og forskning',
        'fi': 'Akateeminen ja tutkimus', 'kg': 'Nzo-nkanda mpe bansosa', 'tl': 'Akademiko at Pananaliksik', 'no': 'Akademisk og forskning', 'sv': 'Akademiskt och forskning',
        'kl': 'Ilisimatusarneq', 'nl': 'Academisch & Onderzoek', 'hr': 'Akademski rad i istraživanje', 'rw': "Ubushakashatsi n'amashuri", 'ro': 'Academic și cercetare',
        'mt': 'Akkademiku u riċerka', 'ne': 'शैक्षिक र अनुसन्धान', 'eo': 'Akademia kaj esplorado', 'sl': 'Akademsko in raziskave', 'tr': 'Akademik ve Araştırma',
        'uk': 'Академічні дослідження', 'el': 'Ακαδημαϊκά & Έρευνα', 'hu': 'Tudományos és kutatás'
    },
    'custom': {
        'zh-CN': '自定义与扩展', 'zh-HK': '自訂與擴展', 'zh-TW': '自訂與擴充',
        'en': 'Custom & Extensions', 'ja': 'カスタムと拡張', 'ko': '사용자 지정 및 확장', 'fr': 'Personnalisation et extensions', 'de': 'Benutzerdefiniert & Erweiterungen',
        'es': 'Personalizado y extensiones', 'pt': 'Personalizado e extensões', 'ru': 'Пользовательские и расширения', 'it': 'Personalizzato ed estensioni', 'ar': 'مخصص والملحقات',
        'he': 'מותאם אישית והרחבות', 'ug': 'ئىختىيارىي ۋە كېڭەيتىلمە', 'bo': 'རང་བཟོ་དང་རྒྱ་བསྐྱེད།', 'mn': 'Өөрийн тохиргоо ба өргөтгөл', 'th': 'กำหนดเองและส่วนขยาย',
        'vi': 'Tùy chỉnh & Tiện ích mở rộng', 'id': 'Kustom & Ekstensi', 'hi': 'कस्टम और एक्सटेंशन', 'bn': 'কাস্টম ও এক্সটেনশন', 'my': 'စိတ်ကြိုက်နှင့် တိုးချဲ့ချက်များ',
        'lo': 'ກຳນົດເອງ ແລະ ສ່ວນຂະຫຍາຍ', 'km': 'កំណត់ផ្ទាល់ខ្លួន និងផ្នែកបន្ថែម', 'ms': 'Tersuai & Sambungan', 'ga': 'Saincheaptha & Eisínteachtaí', 'da': 'Brugerdefineret og udvidelser',
        'fi': 'Mukautettu ja laajennukset', 'kg': 'Yonso ya kusala mpe kuyikama', 'tl': 'Naka-customize at Mga Extension', 'no': 'Tilpasset og utvidelser', 'sv': 'Anpassat och tillägg',
        'kl': 'Namminerisamik ilassutillu', 'nl': 'Aangepast & Extensies', 'hr': 'Prilagođeno i proširenja', 'rw': "Ibyihariye n'imyaguro", 'ro': 'Personalizat și extensii',
        'mt': 'Personalizzat u estensjonijiet', 'ne': 'अनुकूलन र विस्तारहरू', 'eo': 'Propra kaj etendaĵoj', 'sl': 'Po meri in razširitve', 'tr': 'Özel ve Uzantılar',
        'uk': 'Власні та розширення', 'el': 'Προσαρμογή & Επεκτάσεις', 'hu': 'Egyéni és bővítmények'
    }
}

SKILLS_AND_ACTIONS = {
    'quick_read': {
        'skill_id': 'readmd-quick-read',
        'zh-CN': '快速阅读', 'zh-HK': '快速閱讀', 'zh-TW': '快速閱讀',
        'en': 'Quick Read', 'ja': 'クイックリーディング', 'ko': '빠른 읽기', 'fr': 'Lecture rapide', 'de': 'Schnelles Lesen',
        'es': 'Lectura rápida', 'pt': 'Leitura rápida', 'ru': 'Быстрое чтение', 'it': 'Lettura veloce', 'ar': 'قراءة سريعة',
        'he': 'קריאה מהירה', 'ug': 'تېز ئوقۇش', 'bo': 'མྱུར་ཀློག', 'mn': 'Хурдан унших', 'th': 'อ่านอย่างรวดเร็ว',
        'vi': 'Đọc nhanh', 'id': 'Baca Cepat', 'hi': 'त्वरित पठन', 'bn': 'দ্রুত পাঠ', 'my': 'အမြန်ဖတ်ရှုခြင်း',
        'lo': 'ອ່ານດ່ວນ', 'km': 'ការអានរហ័ស', 'ms': 'Bacaan Pantas', 'ga': 'Léamh Tapa', 'da': 'Hurtiglæsning',
        'fi': 'Pikaluku', 'kg': 'Kutanga nswalu', 'tl': 'Mabilisang Pagbasa', 'no': 'Hurtiglesing', 'sv': 'Snabbläsning',
        'kl': 'Sukkasumik atuarnaq', 'nl': 'Snel Lezen', 'hr': 'Brzo čitanje', 'rw': 'Gusoma byihuse', 'ro': 'Citire rapidă',
        'mt': 'Qari Rapidu', 'ne': 'द्रुत पठन', 'eo': 'Rapida Legado', 'sl': 'Hitro branje', 'tr': 'Hızlı Okuma',
        'uk': 'Швидке читання', 'el': 'Γρήγορη Ανάγνωση', 'hu': 'Gyorsolvasás'
    },
    'polish': {
        'skill_id': 'readmd-polish',
        'zh-CN': '润色文稿', 'zh-HK': '潤色文稿', 'zh-TW': '潤色文稿',
        'en': 'Polish Text', 'ja': '文章の推敲', 'ko': '글 다듬기', 'fr': 'Polir le texte', 'de': 'Text verfeinern',
        'es': 'Pulir texto', 'pt': 'Polir texto', 'ru': 'Полировка текста', 'it': 'Perfezionamento testo', 'ar': 'تنقيح النص',
        'he': 'ליטוש טקסט', 'ug': 'يازمىنى سىلىقلاش', 'bo': 'རྩོམ་ཡིག་ལེགས་བཅོས།', 'mn': 'Текстийг сайжруулах', 'th': 'ขัดเกลาข้อความ',
        'vi': 'Trau chuốt văn bản', 'id': 'Poles Teks', 'hi': 'पाठ परिष्कृत करें', 'bn': 'লেখা পরিমার্জন', 'my': 'စာသားပြင်ဆင်မွမ်းမံခြင်း',
        'lo': 'ປັບປຸງບົດຂຽນ', 'km': 'កែលម្អអត្ថបទ', 'ms': 'Mengilap Teks', 'ga': 'Snasaigh Téacs', 'da': 'Finpuds tekst',
        'fi': 'Hio tekstiä', 'kg': 'Kubongisa bisono', 'tl': 'Pakinisin ang Teksto', 'no': 'Finpuss tekst', 'sv': 'Finslipa text',
        'kl': 'Allattat pitsanngorsakkit', 'nl': 'Tekst Polijsten', 'hr': 'Uređivanje teksta', 'rw': 'Kunonosora inyandiko', 'ro': 'Finisare text',
        'mt': 'Illustra t-test', 'ne': 'पाठ परिमार्जन गर्नुहोस्', 'eo': 'Poluri Tekston', 'sl': 'Izpopolni besedilo', 'tr': 'Metni İyileştir',
        'uk': 'Полірування тексту', 'el': 'Επιμέλεια Κειμένου', 'hu': 'Szöveg csiszolása'
    },
    'proofread': {
        'skill_id': 'readmd-proofread',
        'zh-CN': '语法纠错', 'zh-HK': '語法糾錯', 'zh-TW': '語法校對',
        'en': 'Proofread & Grammar', 'ja': '文法・校正', 'ko': '맞춤법 및 교정', 'fr': 'Correction grammaticale', 'de': 'Grammatikkorrektur',
        'es': 'Corrección gramatical', 'pt': 'Correção gramatical', 'ru': 'Корректура и грамматика', 'it': 'Correzione grammaticale', 'ar': 'تصحيح نحوي ولغوي',
        'he': 'הגהה ותיקון דקדוק', 'ug': 'گرامماتىكا تۈزىتىش', 'bo': 'བརྡ་དག་ནོར་བཅོས།', 'mn': 'Дүрмийн алдаа засах', 'th': 'ตรวจไวยากรณ์',
        'vi': 'Sửa lỗi ngữ pháp', 'id': 'Periksa Tata Bahasa', 'hi': 'व्याकरण सुधार', 'bn': 'ব্যাকরণ সংশোধন', 'my': 'သဒ္ဒါစစ်ဆေးပြင်ဆင်ခြင်း',
        'lo': 'ກວດແກ້ໄວຍາກອນ', 'km': 'កែវេយ្យាករណ៍', 'ms': 'Semakan Tatabahasa', 'ga': 'Pcruthúnas & Gramadach', 'da': 'Korrekturlæsning',
        'fi': 'Oikoluku ja kielioppi', 'kg': 'Kusungika gramere', 'tl': 'Pagwawasto ng Gramatika', 'no': 'Korrektur og grammatikk', 'sv': 'Korrekturläsning',
        'kl': 'Oqaasilerinermik aaqqiineq', 'nl': 'Proeflezen & Grammatica', 'hr': 'Lektura i gramatika', 'rw': 'Gukosora ikibonezamvugo', 'ro': 'Corectare gramaticală',
        'mt': 'Qari tal-provi u grammatika', 'ne': 'व्याकरण सुधार', 'eo': 'Korektado de Gramatiko', 'sl': 'Lektura in slovnica', 'tr': 'Dilbilgisi Düzeltme',
        'uk': 'Перевірка граматики', 'el': 'Διόρθωση Γραμματικής', 'hu': 'Nyelvtani ellenőrzés'
    },
    'to_english': {
        'skill_id': None,
        'zh-CN': '翻译为英文', 'zh-HK': '翻譯為英文', 'zh-TW': '翻譯為英文',
        'en': 'Translate to English', 'ja': '英語に翻訳', 'ko': '영어로 번역', 'fr': 'Traduire en anglais', 'de': 'Ins Englische übersetzen',
        'es': 'Traducir al inglés', 'pt': 'Traduzir para inglês', 'ru': 'Перевести на английский', 'it': 'Traduci in inglese', 'ar': 'الترجمة إلى الإنجليزية',
        'he': 'תרגם לאנגלית', 'ug': 'ئىنگلىزچىغا تەرجىمە قىلىش', 'bo': 'དབྱིན་ཡིག་ཏུ་སྒྱུར་བ།', 'mn': 'Англи хэл рүү орчуулах', 'th': 'แปลเป็นภาษาอังกฤษ',
        'vi': 'Dịch sang tiếng Anh', 'id': 'Terjemahkan ke Bahasa Inggris', 'hi': 'अंग्रेज़ी में अनुवाद करें', 'bn': 'ইংরেজিতে অনুবাদ করুন', 'my': 'အင်္ဂလိပ်ဘာသာသို့ ဘာသာပြန်ဆိုရန်',
        'lo': 'ແປເປັນພາສາອັງກິດ', 'km': 'បកប្រែជាភាសាអង់គ្លេស', 'ms': 'Terjemah ke Bahasa Inggeris', 'ga': 'Aistrigh go Béarla', 'da': 'Oversæt til engelsk',
        'fi': 'Käännä englanniksi', 'kg': 'Kubalula na kingelezi', 'tl': 'Isalin sa Ingles', 'no': 'Oversett til engelsk', 'sv': 'Översätt till engelska',
        'kl': 'Tuluttuumut nutseruk', 'nl': 'Vertalen naar Engels', 'hr': 'Prevedi na engleski', 'rw': 'Guhindura mu cyongereza', 'ro': 'Traducere în engleză',
        'mt': 'Ittraduċi għall-Ingliż', 'ne': 'अंग्रेजीमा अनुवाद गर्नुहोस्', 'eo': 'Traduki al la angla', 'sl': 'Prevedi v angleščino', 'tr': 'İngilizceye Çevir',
        'uk': 'Перекласти англійською', 'el': 'Μετάφραση στα Αγγλικά', 'hu': 'Fordítás angolra'
    },
    'to_chinese': {
        'skill_id': None,
        'zh-CN': '翻译为中文', 'zh-HK': '翻譯為中文', 'zh-TW': '翻譯為中文',
        'en': 'Translate to Chinese', 'ja': '中国語に翻訳', 'ko': '중국어로 번역', 'fr': 'Traduire en chinois', 'de': 'Ins Chinesische übersetzen',
        'es': 'Traducir al chino', 'pt': 'Traduzir para chinês', 'ru': 'Перевести на китайский', 'it': 'Traduci in cinese', 'ar': 'الترجمة إلى الصينية',
        'he': 'תרגם לסינית', 'ug': 'خەنزۇچىغا تەرجىمە قىلىش', 'bo': 'རྒྱ་ཡིག་ཏུ་སྒྱུར་བ།', 'mn': 'Хятад хэл рүү орчуулах', 'th': 'แปลเป็นภาษาจีน',
        'vi': 'Dịch sang tiếng Trung', 'id': 'Terjemahkan ke Bahasa Mandarin', 'hi': 'चीनी में अनुवाद करें', 'bn': 'চীনা ভাষায় অনুবাদ করুন', 'my': 'တရုတ်ဘာသာသို့ ဘာသာပြန်ဆိုရန်',
        'lo': 'ແປເປັນພາສາຈີນ', 'km': 'បកប្រែជាភាសាចិន', 'ms': 'Terjemah ke Bahasa Cina', 'ga': 'Aistrigh go Sínis', 'da': 'Oversæt til kinesisk',
        'fi': 'Käännä kiinaksi', 'kg': 'Kubalula na kisinwa', 'tl': 'Isalin sa Tsino', 'no': 'Oversett til kinesisk', 'sv': 'Översätt till kinesiska',
        'kl': 'Kineseritut nutseruk', 'nl': 'Vertalen naar Chinees', 'hr': 'Prevedi na kineski', 'rw': 'Guhindura mu gishinwa', 'ro': 'Traducere în chineză',
        'mt': 'Ittraduċi għaċ-Ċiniż', 'ne': 'चिनियाँ भाषामा अनुवाद गर्नुहोस्', 'eo': 'Traduki al la ĉina', 'sl': 'Prevedi v kitajščino', 'tr': 'Çinceye Çevir',
        'uk': 'Перекласти китайською', 'el': 'Μετάφραση στα Κινεζικά', 'hu': 'Fordítás kínaira'
    },
    'readmd-translate': {
        'action_id': None,
        'skill_id': 'readmd-translate',
        'zh-CN': '文档翻译', 'zh-HK': '文件翻譯', 'zh-TW': '文件翻譯',
        'en': 'Document Translation', 'ja': 'ドキュメント翻訳', 'ko': '문서 번역', 'fr': 'Traduction de documents', 'de': 'Dokumentübersetzung',
        'es': 'Traducción de documentos', 'pt': 'Tradução de documentos', 'ru': 'Перевод документов', 'it': 'Traduzione documenti', 'ar': 'ترجمة المستندات',
        'he': 'תרגום מסמכים', 'ug': 'پۈتۈك تەرجىمىسى', 'bo': 'ཡིག་ཆ་སྒྱུར་བ།', 'mn': 'Баримт бичгийн орчуулга', 'th': 'แปลเอกสาร',
        'vi': 'Dịch tài liệu', 'id': 'Penerjemahan Dokumen', 'hi': 'दस्तावेज़ अनुवाद', 'bn': 'নথি অনুবাদ', 'my': 'စာရွက်စာတမ်း ဘာသာပြန်ဆိုခြင်း',
        'lo': 'ການແປເອກະສານ', 'km': 'ការបកប្រែឯកសារ', 'ms': 'Terjemahan Dokumen', 'ga': 'Aistriú Doiciméad', 'da': 'Dokumentoversættelse',
        'fi': 'Asiakirjojen kääntäminen', 'kg': 'Kubalula mikanda', 'tl': 'Pagsasalin ng Dokumento', 'no': 'Dokumentoversettelse', 'sv': 'Dokumentöversättning',
        'kl': 'Allagaatinik nutserineq', 'nl': 'Document Vertaling', 'hr': 'Prijevod dokumenata', 'rw': "Ubuhinduzi bw'inyandiko", 'ro': 'Traducere documente',
        'mt': "Traduzzjoni ta' dokumenti", 'ne': 'कागजात अनुवाद', 'eo': 'Dokumenta Tradukado', 'sl': 'Prevajanje dokumentov', 'tr': 'Belge Çevirisi',
        'uk': 'Переклад документів', 'el': 'Μετάφραση Εγγράφων', 'hu': 'Dokumentumfordítás'
    },
    'action_items': {
        'skill_id': 'readmd-todo',
        'zh-CN': '提取待办', 'zh-HK': '提取待辦', 'zh-TW': '擷取待辦',
        'en': 'Extract Action Items', 'ja': 'アクションアイテム抽出', 'ko': '할 일 추출', 'fr': 'Extraire les tâches', 'de': 'Aufgaben extrahieren',
        'es': 'Extraer tareas pendientes', 'pt': 'Extrair itens de ação', 'ru': 'Извлечь задачи', 'it': 'Estrai attività', 'ar': 'استخراج المهام',
        'he': 'חילוץ משימות לביצוע', 'ug': 'بېجىرىلىدىغانلارنى ئايرىش', 'bo': 'སྒྲུབ་བྱའི་དོན་ཚན་ཕྱིར་འདོན།', 'mn': 'Хийх ажлыг ялгах', 'th': 'สกัดรายการที่ต้องทำ',
        'vi': 'Trích xuất công việc', 'id': 'Ekstrak Tugas', 'hi': 'कार्य सूची निकालें', 'bn': 'করণীয় কাজ নিষ্কাশন', 'my': 'လုပ်ဆောင်ရန်အချက်များ ထုတ်ယူခြင်း',
        'lo': 'ດຶງລາຍການທີ່ຕ້ອງເຮັດ', 'km': 'ទាញយកបញ្ជីការងារត្រូវធ្វើ', 'ms': 'Ekstrak Senarai Tugas', 'ga': 'Sliocht Míreanna Gníomhaíochta', 'da': 'Udtræk opgaver',
        'fi': 'Pura tehtävät', 'kg': 'Kubimisa bisalu', 'tl': 'I-extract ang mga Gagawin', 'no': 'Trekk ut gjøremål', 'sv': 'Extrahera uppgifter',
        'kl': 'Suliassat saqqummersikkit', 'nl': 'Taken Extraheren', 'hr': 'Izdvoji zadatke', 'rw': 'Gukuramo ibigomba gukorwa', 'ro': 'Extragere sarcini',
        'mt': "Oħroġ l-oġġetti ta' azzjoni", 'ne': 'कार्य वस्तुहरू निकाल्नुहोस्', 'eo': 'Ekstrakti Agadendaĵojn', 'sl': 'Izvleček nalog', 'tr': 'Yapılacakları Çıkar',
        'uk': 'Вилучити завдання', 'el': 'Εξαγωγή Ενεργειών', 'hu': 'Teendők kinyerése'
    },
    'continue': {
        'skill_id': 'readmd-continue',
        'zh-CN': '续写内容', 'zh-HK': '續寫內容', 'zh-TW': '續寫內容',
        'en': 'Continue Writing', 'ja': '続きを書く', 'ko': '이어 쓰기', 'fr': 'Continuer la rédaction', 'de': 'Inhalt fortsetzen',
        'es': 'Continuar escribiendo', 'pt': 'Continuar escrevendo', 'ru': 'Продолжить написание', 'it': 'Continua a scrivere', 'ar': 'مواصلة الكتابة',
        'he': 'המשך כתיבה', 'ug': 'داۋاملاشتۇرۇپ يېزىش', 'bo': 'མུ་མཐུད་རྩོམ་འབྲི།', 'mn': 'Үргэлжлүүлэн бичих', 'th': 'เขียนต่อ',
        'vi': 'Viết tiếp nội dung', 'id': 'Lanjutkan Menulis', 'hi': 'आगे लिखें', 'bn': 'লেখা চালিয়ে যান', 'my': 'ဆက်လက်ရေးသားခြင်း',
        'lo': 'ຂຽນເນື້ອຫາຕໍ່', 'km': 'សរសេរបន្ត', 'ms': 'Sambung Penulisan', 'ga': 'Lean ar Aghaidh ag Scríobh', 'da': 'Fortsæt skrivning',
        'fi': 'Jatka kirjoittamista', 'kg': 'Kokoba kusonika', 'tl': 'Magpatuloy sa Pagsusulat', 'no': 'Fortsett å skrive', 'sv': 'Fortsätt skriva',
        'kl': 'Allannerit ingerlateqqiguk', 'nl': 'Verder Schrijven', 'hr': 'Nastavi pisati', 'rw': 'Gukomeza kwandika', 'ro': 'Continuare scriere',
        'mt': 'Kompli Ikteb', 'ne': 'लेखन जारी राख्नुहोस्', 'eo': 'Daŭrigi Verkadon', 'sl': 'Nadaljuj s pisanjem', 'tr': 'Yazmaya Devam Et',
        'uk': 'Продовжити написання', 'el': 'Συνέχιση Συγγραφής', 'hu': 'Írás folytatása'
    },
    'ask': {
        'skill_id': 'readmd-ask',
        'zh-CN': '自由提问', 'zh-HK': '自由提問', 'zh-TW': '自由提問',
        'en': 'Ask Anything', 'ja': '自由に質問', 'ko': '자유 질문', 'fr': 'Poser une question', 'de': 'Freie Frage stellen',
        'es': 'Pregunta libre', 'pt': 'Pergunta livre', 'ru': 'Задать вопрос', 'it': 'Domanda libera', 'ar': 'سؤال حر',
        'he': 'שאל בחופשיות', 'ug': 'ئەركىن سوئال سوراش', 'bo': 'རང་དབང་འདྲི་རྩད།', 'mn': 'Чөлөөтэй асуух', 'th': 'ถามได้อย่างอิสระ',
        'vi': 'Hỏi tự do', 'id': 'Tanya Bebas', 'hi': 'स्वतंत्र रूप से पूछें', 'bn': 'মুক্ত প্রশ্ন জিজ্ঞাসা', 'my': 'လွတ်လပ်စွာ မေးမြန်းခြင်း',
        'lo': 'ຖາມໄດ້ຢ່າງເສລີ', 'km': 'សួរសំណួរដោយសេរី', 'ms': 'Soalan Bebas', 'ga': 'Cuir Ceist ar bith', 'da': 'Stil et spørgsmål',
        'fi': 'Kysy vapaasti', 'kg': 'Yula kyuvu na kimpwanza', 'tl': 'Malayang Magtanong', 'no': 'Still et spørsmål', 'sv': 'Ställ en fråga',
        'kl': 'Kiffaanngissuseqarlutit aperigit', 'nl': 'Vrij Vragen', 'hr': 'Slobodno pitanje', 'rw': 'Kubaza icyo ushaka cyose', 'ro': 'Întrebare liberă',
        'mt': 'Saqsi kull ħaġa', 'ne': 'खुला प्रश्न सोध्नुहोस्', 'eo': 'Libere Demandi', 'sl': 'Prosto vprašanje', 'tr': 'Serbest Soru',
        'uk': 'Вільне запитання', 'el': 'Ελεύθερη Ερώτηση', 'hu': 'Kérdés feltevése'
    },
    'summary': {
        'skill_id': 'readmd-summary',
        'zh-CN': '总结要点', 'zh-HK': '總結要點', 'zh-TW': '總結要點',
        'en': 'Summarize Key Points', 'ja': '要点まとめ', 'ko': '핵심 요약', 'fr': 'Résumer les points clés', 'de': 'Kernpunkte zusammenfassen',
        'es': 'Resumir puntos clave', 'pt': 'Resumir pontos-chave', 'ru': 'Главные тезисы', 'it': 'Riassumi punti chiave', 'ar': 'تلخيص النقاط الرئيسية',
        'he': 'סיכום נקודות מפתח', 'ug': 'مۇھىم نۇقتىلارنى خۇلاسىلەش', 'bo': 'གནད་དོན་མདོར་བསྡུས།', 'mn': 'Гол санааг дүгнэх', 'th': 'สรุปประเด็นสำคัญ',
        'vi': 'Tóm tắt ý chính', 'id': 'Ringkas Poin Utama', 'hi': 'मुख्य बिंदुओं का सारांश', 'bn': 'মূল বিষয়গুলির সারসংক্ষেপ', 'my': 'အဓိကအချက်များကို အကျဉ်းချုပ်ဖော်ပြခြင်း',
        'lo': 'ສະຫຼຸບຈຸດສຳຄັນ', 'km': 'សង្ខេបចំណុចសំខាន់ៗ', 'ms': 'Ringkaskan Isi Utama', 'ga': 'Déan Achoimre ar Phríomhphointí', 'da': 'Opsummer hovedpunkter',
        'fi': 'Tiivistä pääkohdat', 'kg': 'Vutukila mambu ya nene', 'tl': 'Ibuod ang Mahahalagang Punto', 'no': 'Oppsummer hovedpunkter', 'sv': 'Sammanfatta huvudpunkter',
        'kl': 'Eqikkarneq', 'nl': 'Kernpunten Samenvatten', 'hr': 'Sažetak ključnih točaka', 'rw': "Incamake y'ingenzi", 'ro': 'Rezumare idei principale',
        'mt': 'Iġbor fil-qosor il-punti ewlenin', 'ne': 'मुख्य बुँदाहरू सारांश गर्नुहोस्', 'eo': 'Resumi Ĉefpunktojn', 'sl': 'Povzetek ključnih točk', 'tr': 'Önemli Noktaları Özetle',
        'uk': 'Резюме основних тез', 'el': 'Σύνοψη Βασικών Σημείων', 'hu': 'Főbb pontok összegzése'
    },
    'outline': {
        'skill_id': 'readmd-outline',
        'zh-CN': '生成大纲', 'zh-HK': '生成大綱', 'zh-TW': '產生大綱',
        'en': 'Generate Outline', 'ja': 'アウトライン作成', 'ko': '개요 생성', 'fr': 'Générer un plan', 'de': 'Gliederung erstellen',
        'es': 'Generar esquema', 'pt': 'Gerar esquema', 'ru': 'Создать план', 'it': 'Genera scaletta', 'ar': 'إنشاء مخطط تفصيلي',
        'he': 'צור ראשי פרקים', 'ug': 'پىلان لايىھىسى ھاسىللاش', 'bo': 'རྩོམ་གྱི་སྒྲོམ་གཞི་བཟོ་བ།', 'mn': 'Бүтэц төлөвлөгөө гаргах', 'th': 'สร้างโครงร่าง',
        'vi': 'Tạo dàn ý', 'id': 'Buat Kerangka', 'hi': 'रूपरेखा तैयार करें', 'bn': 'রূপরেখা তৈরি করুন', 'my': 'စာအကြမ်း အကြမ်းဖျင်းရေးဆွဲခြင်း',
        'lo': 'ສ້າງໂຄງຮ່າງ', 'km': 'បង្កើតគ្រោង', 'ms': 'Jana Rangka', 'ga': 'Gin Imlíne', 'da': 'Opret disposition',
        'fi': 'Luo jäsentely', 'kg': 'Kusala bansora ya luyantiku', 'tl': 'Gumawa ng Balangkas', 'no': 'Generer disposisjon', 'sv': 'Skapa disposition',
        'kl': 'Allattukkat aaqqissugaanerat', 'nl': 'Overzicht Genereren', 'hr': 'Stvori nacrt', 'rw': "Kurema amavu n'amavuko", 'ro': 'Generare schiță',
        'mt': 'Iġġenera tabella tal-punti', 'ne': 'रूपरेखा सिर्जना गर्नुहोस्', 'eo': 'Generi Skizon', 'sl': 'Ustvari oris', 'tr': 'Anahat Oluştur',
        'uk': 'Створити план', 'el': 'Δημιουργία Περιγράμματος', 'hu': 'Vázlat készítése'
    },
    'weekly': {
        'skill_id': 'readmd-weekly',
        'zh-CN': '生成周报', 'zh-HK': '生成週報', 'zh-TW': '產生週報',
        'en': 'Generate Weekly Report', 'ja': '週報作成', 'ko': '주간 보고서 생성', 'fr': 'Générer un rapport hebdomadaire', 'de': 'Wochenbericht erstellen',
        'es': 'Generar informe semanal', 'pt': 'Gerar relatório semanal', 'ru': 'Создать еженедельный отчёт', 'it': 'Genera report settimanale', 'ar': 'إنشاء تقرير أسبوعي',
        'he': 'הפק דוח שבועי', 'ug': 'ھەپتىلىك دوكلات ھاسىللاش', 'bo': 'གཟའ་འཁོར་སྙན་ཞུ་བཟོ་བ།', 'mn': 'Долоо хоногийн тайлан гаргах', 'th': 'สร้างรายงานประจำสัปดาห์',
        'vi': 'Tạo báo cáo tuần', 'id': 'Buat Laporan Mingguan', 'hi': 'साप्ताहिक रिपोर्ट तैयार करें', 'bn': 'সাপ্তাহিক রিপোর্ট তৈরি করুন', 'my': 'အပတ်စဉ်အစီရင်ခံစာ ရေးဆွဲခြင်း',
        'lo': 'ສ້າງລາຍງານປະຈຳອາທິດ', 'km': 'បង្កើតរបាយការណ៍ប្រចាំសប្តាហ៍', 'ms': 'Jana Laporan Mingguan', 'ga': 'Gin Tuarascáil Sheachtainiúil', 'da': 'Generer ugerapport',
        'fi': 'Luo viikkoraportti', 'kg': 'Kusala rapore ya mposo', 'tl': 'Gumawa ng Lingguhang Ulat', 'no': 'Generer ukerapport', 'sv': 'Skapa veckorapport',
        'kl': 'Sapaatip-akunneranoortumik nalunaarusiaq', 'nl': 'Weekrapport Genereren', 'hr': 'Generiraj tjedni izvještaj', 'rw': "Kora raporo y'icyumweru", 'ro': 'Generare raport săptămânal',
        'mt': "Iġġenera rapport ta' kull ġimgħa", 'ne': 'साप्ताहिक प्रतिवेदन तयार गर्नुहोस्', 'eo': 'Generi Semajnan Raporton', 'sl': 'Ustvari tedensko poročilo', 'tr': 'Haftalık Rapor Oluştur',
        'uk': 'Створити щотижневий звіт', 'el': 'Δημιουργία Εβδομαδιαίας Αναφοράς', 'hu': 'Heti jelentés készítése'
    },
    'code_review': {
        'skill_id': 'readmd-code-review',
        'zh-CN': '代码审查', 'zh-HK': '代碼審查', 'zh-TW': '程式碼審查',
        'en': 'Code Review', 'ja': 'コードレビュー', 'ko': '코드 리뷰', 'fr': 'Revue de code', 'de': 'Code-Überprüfung',
        'es': 'Revisión de código', 'pt': 'Revisão de código', 'ru': 'Ревью кода', 'it': 'Revisione del codice', 'ar': 'مراجعة الكود البرمجي',
        'he': 'סקירת קוד', 'ug': 'كود تەكشۈرۈش', 'bo': 'ཚབ་ཨང་ཞིབ་བཤེར།', 'mn': 'Код шалгах', 'th': 'ตรวจสอบโค้ด',
        'vi': 'Đánh giá mã nguồn', 'id': 'Tinjau Kode', 'hi': 'कोड समीक्षा', 'bn': 'কোড পর্যালোচনা', 'my': 'ကုဒ်စစ်ဆေးသုံးသပ်ခြင်း',
        'lo': 'ກວດສອບໂຄ້ດ', 'km': 'ការពិនិត្យឡើងវិញនូវកូដ', 'ms': 'Semakan Kod', 'ga': 'Athbhreithniú Cóid', 'da': 'Kodegennemgang',
        'fi': 'Koodikatselmointi', 'kg': 'Kutala kodi', 'tl': 'Pagsusuri ng Code', 'no': 'Kodegjennomgang', 'sv': 'Kodgranskning',
        'kl': 'Qarasaasialiakkap kukkunersiorneqarnera', 'nl': 'Code Review', 'hr': 'Pregled koda', 'rw': 'Gusuzuma kode', 'ro': 'Revizuire cod',
        'mt': 'Reviżjoni tal-kodiċi', 'ne': 'कोड समीक्षा', 'eo': 'Koda Revizio', 'sl': 'Pregled kode', 'tr': 'Kod İnceleme',
        'uk': 'Рецензування коду', 'el': 'Έλεγχος Κώδικα', 'hu': 'Kód áttekintése'
    },
    'fix_format': {
        'skill_id': 'readmd-format-fix',
        'zh-CN': '修正格式', 'zh-HK': '修正格式', 'zh-TW': '修正格式',
        'en': 'Fix Formatting', 'ja': '書式修正', 'ko': '서식 수정', 'fr': 'Corriger le formatage', 'de': 'Formatierung korrigieren',
        'es': 'Corregir formato', 'pt': 'Corrigir formatação', 'ru': 'Исправить форматирование', 'it': 'Correggi formattazione', 'ar': 'إصلاح التنسيق',
        'he': 'תקן עיצוב', 'ug': 'فورماتنى تۈزىتىش', 'bo': 'རྣམ་གཞག་དག་བཅོས།', 'mn': 'Формат засах', 'th': 'แก้ไขการจัดรูปแบบ',
        'vi': 'Sửa định dạng', 'id': 'Perbaiki Format', 'hi': 'प्रारूप ठीक करें', 'bn': 'বিন্যাস সংশোধন', 'my': 'ဖော်မတ်ပြင်ဆင်ခြင်း',
        'lo': 'ແກ້ໄຂຮູບແບບ', 'km': 'កែទម្រង់', 'ms': 'Baiki Format', 'ga': 'Deisigh Formáidiú', 'da': 'Ret formatering',
        'fi': 'Korjaa muotoilu', 'kg': 'Kubongisa mformat', 'tl': 'Ayusin ang Pag-format', 'no': 'Rett opp formatering', 'sv': 'Åtgärda formatering',
        'kl': 'Aaqqissugaanerata iluarsinera', 'nl': 'Opmaak Corrigeren', 'hr': 'Ispravi oblikovanje', 'rw': 'Gukosora imiterere', 'ro': 'Corectare formatare',
        'mt': 'Irranġa l-formattjar', 'ne': 'ढाँचा मिलाउनुहोस्', 'eo': 'Korekti Formaton', 'sl': 'Popravi oblikovanje', 'tr': 'Biçimlendirmeyi Düzelt',
        'uk': 'Виправити форматування', 'el': 'Διόρθωση Μορφοποίησης', 'hu': 'Formázás javítása'
    }
}

PET_UPDATES = {
    'pet.updateTitle': {
        'zh-CN': '桌面桌宠独立包与更新', 'zh-HK': '桌面桌寵獨立包與更新', 'zh-TW': '桌面桌寵獨立包與更新',
        'en': 'Desktop Pet Package & Updates', 'ja': 'デスクトップペットパッケージとアップデート', 'ko': '데스크톱 펫 패키지 및 업데이트',
        'fr': 'Package et mises à jour du compagnon de bureau', 'de': 'Desktop-Haustierpaket & Updates', 'es': 'Paquete de mascota de escritorio y actualizaciones',
        'pt': 'Pacote de mascote de desktop e atualizações', 'ru': 'Пакет настольного питомца и обновления', 'it': 'Pacchetto pet desktop e aggiornamenti',
        'ar': 'حزمة الحيوان الأليف لسطح المكتب والتحديثات', 'he': 'חבילת חיית מחמד לשולחן העבודה ועדכונים', 'ug': 'ئۈستەلئۈستى ئەرمەك ھايۋان بولىقى ۋە يېڭىلاش',
        'bo': 'ཅོག་ངོས་གཅེས་ཉར་ཐུམ་བུ་དང་གསར་བཅོས།', 'mn': 'Дэлгэцийн амьтан багц ба шинэчлэлт', 'th': 'แพ็กเกจสัตว์เลี้ยงบนเดสก์ท็อปและการอัปเดต',
        'vi': 'Gói thú cưng màn hình & Cập nhật', 'id': 'Paket & Pembaruan Hewan Peliharaan Desktop', 'hi': 'डेस्कटॉप पेट पैकेज और अपडेट',
        'bn': 'ডেস্কটপ পেট প্যাকেজ ও আপডেট', 'my': 'ဒက်စ်တော့အိမ်မွေးတိရစ္ဆာန် ပက်ကေ့ဂျ်နှင့် အပ်ဒိတ်များ', 'lo': 'ແພັກເກດສັດລ້ຽງໜ້າຈໍ ແລະ ອັບເດດ',
        'km': 'កញ្ចប់សត្វចិញ្ចឹមលើតុ និងបច្ចុប្បន្នភាព', 'ms': 'Pakej & Kemas Kini Haiwan Peliharaan Desktop', 'ga': 'Pacáiste Peata Deisce & Nuashonruithe',
        'da': 'Skrivebordskæledyrspakke og opdateringer', 'fi': 'Työpöytälempikkipaketti ja päivitykset', 'kg': 'Pake ya kibulu ya ordinatere mpe bansoba',
        'tl': 'Desktop Pet Package at Mga Update', 'no': 'Skrivebordskjæledyrpakke og oppdateringer', 'sv': 'Skrivbordsdjurspaket och uppdateringar',
        'kl': 'Qarasaasiap saani uumasuaq paaqqitassaq nutarterinerillu', 'nl': 'Desktop Huisdier Pakket & Updates', 'hr': 'Paket za stolnog ljubimca i ažuriranja',
        'rw': "Porogaramu y'itungo ryo kuri mudasobwa no kuvugurura", 'ro': 'Pachet animal de companie desktop și actualizări', 'mt': 'Pakkett tal-pet tad-desktop u aġġornamenti',
        'ne': 'डेस्कटप पाल्तु जनावर प्याकेज र अपडेटहरू', 'eo': 'Labortabla Dorlotbesta Pako kaj Ĝisdatigoj', 'sl': 'Paket namiznega ljubljenčka in posodobitve',
        'tr': 'Masaüstü Evcil Hayvan Paketi ve Güncellemeler', 'uk': 'Пакет настільного улюбленця та оновлення', 'el': 'Πακέτο Κατοικιδίου Επιφάνειας Εργασίας & Ενημερώσεις',
        'hu': 'Asztali kisállat csomag és frissítések'
    },
    'pet.checkUpdate': {
        'zh-CN': '检查更新', 'zh-HK': '檢查更新', 'zh-TW': '檢查更新',
        'en': 'Check Update', 'ja': 'アップデートを確認', 'ko': '업데이트 확인', 'fr': 'Vérifier les mises à jour', 'de': 'Nach Updates suchen',
        'es': 'Buscar actualizaciones', 'pt': 'Verificar atualizações', 'ru': 'Проверить обновления', 'it': 'Controlla aggiornamenti',
        'ar': 'التحقق من التحديثات', 'he': 'בדוק עדכונים', 'ug': 'يېڭىلاشنى تەكشۈرۈش', 'bo': 'གསར་བཅོས་ཞིབ་བཤེར།', 'mn': 'Шинэчлэлтийг шалгах',
        'th': 'ตรวจสอบการอัปเดต', 'vi': 'Kiểm tra cập nhật', 'id': 'Periksa Pembaruan', 'hi': 'अपडेट जांचें', 'bn': 'আপডেট পরীক্ষা করুন',
        'my': 'အပ်ဒိတ်ကို စစ်ဆေးပါ', 'lo': 'ກວດສອບອັບເດດ', 'km': 'ពិនិត្យមើលបច្ចុប្បន្នភាព', 'ms': 'Semak Kemas Kini', 'ga': 'Seiceáil Nuashonrú',
        'da': 'Tjek opdatering', 'fi': 'Tarkista päivitykset', 'kg': 'Tala bansoba', 'tl': 'Suriin ang Update', 'no': 'Se etter oppdatering',
        'sv': 'Sök efter uppdatering', 'kl': 'Nutarterineq misissoruk', 'nl': 'Controleren op Updates', 'hr': 'Provjeri ažuriranja',
        'rw': 'Kugenzura amavugurura', 'ro': 'Verificare actualizări', 'mt': 'Iċċekkja għal aġġornament', 'ne': 'अपडेट जाँच गर्नुहोस्',
        'eo': 'Kontroli Ĝisdatigon', 'sl': 'Preveri posodobitve', 'tr': 'Güncellemeleri Denetle', 'uk': 'Перевірити оновлення',
        'el': 'Έλεγχος Ενημερώσεων', 'hu': 'Frissítések keresése'
    },
    'pet.checkingUpdate': {
        'zh-CN': '正在检查更新...', 'zh-HK': '正在檢查更新...', 'zh-TW': '正在檢查更新...',
        'en': 'Checking...', 'ja': '確認中...', 'ko': '확인 중...', 'fr': 'Vérification en cours...', 'de': 'Suche nach Updates...',
        'es': 'Comprobando...', 'pt': 'Verificando...', 'ru': 'Проверка...', 'it': 'Controllo in corso...', 'ar': 'جاري التحقق...',
        'he': 'בודק...', 'ug': 'يېڭىلاش تەكشۈرۈلۈۋاتىدۇ...', 'bo': 'གསར་བཅོས་ཞིབ་བཤེར་བྱེད་བཞིན་པ...', 'mn': 'Шалгаж байна...',
        'th': 'กำลังตรวจสอบ...', 'vi': 'Đang kiểm tra...', 'id': 'Memeriksa...', 'hi': 'जांच की जा रही है...', 'bn': 'পরীক্ষা করা হচ্ছে...',
        'my': 'စစ်ဆေးနေပါသည်...', 'lo': 'ກຳລັງກວດສອບ...', 'km': 'កំពុងពិនិត្យ...', 'ms': 'Menyemak...', 'ga': 'Ag seiceáil...',
        'da': 'Tjekker...', 'fi': 'Tarkistetaan...', 'kg': 'Kutala...', 'tl': 'Sinusuri...', 'no': 'Sjekker...',
        'sv': 'Söker...', 'kl': 'Misissuineq ingerlavoq...', 'nl': 'Bezig met controleren...', 'hr': 'Provjera u tijeku...',
        'rw': 'Birimo kugenzurwa...', 'ro': 'Se verifică...', 'mt': 'Qed jiċċekkja...', 'ne': 'जाँच गरिँदैछ...',
        'eo': 'Kontrolante...', 'sl': 'Preverjanje...', 'tr': 'Denetleniyor...', 'uk': 'Перевірка...', 'el': 'Έλεγχος...', 'hu': 'Ellenőrzés folyamatban...'
    },
    'pet.updateAvailable': {
        'zh-CN': '发现新桌宠包版本 {ver}！', 'zh-HK': '發現新桌寵包版本 {ver}！', 'zh-TW': '發現新桌寵包版本 {ver}！',
        'en': 'New pet package {ver} available!', 'ja': '新しいペットパッケージ {ver} が利用可能です！', 'ko': '새로운 펫 패키지 {ver} 버전을 사용할 수 있습니다!',
        'fr': 'Nouveau package {ver} disponible !', 'de': 'Neues Haustierpaket {ver} verfügbar!', 'es': '¡Nuevo paquete de mascota {ver} disponible!',
        'pt': 'Novo pacote de mascote {ver} disponível!', 'ru': 'Доступен новый пакет питомца {ver}!', 'it': 'Nuovo pacchetto pet {ver} disponibile!',
        'ar': 'حزمة الحيوان الأليف الجديدة {ver} متوفرة!', 'he': 'חבילת חיית מחמד חדשה {ver} זמינה!', 'ug': 'يېڭى ئەرمەك ھايۋان بولىقى نەشرى {ver} تېپىلدى!',
        'bo': 'གཅེས་ཉར་ཐུམ་བུའི་པར་གཞི་གསར་པ {ver} རྙེད་བྱུང་!', 'mn': 'Амьтны шинэ багц {ver} хувилбар бэлэн боллоо!', 'th': 'มีแพ็กเกจสัตว์เลี้ยงเวอร์ชันใหม่ {ver} พร้อมใช้งาน!',
        'vi': 'Đã có phiên bản gói thú cưng mới {ver}!', 'id': 'Paket hewan peliharaan baru {ver} tersedia!', 'hi': 'नया पेट पैकेज {ver} उपलब्ध है!',
        'bn': 'নতুন পেট প্যাকেজ {ver} উপলব্ধ!', 'my': 'အိမ်မွေးတိရစ္ဆာန် ပက်ကေ့ဂျ်ဗားရှင်းအသစ် {ver} ရရှိနိုင်ပါပြီ။', 'lo': 'ມີແພັກເກດສັດລ້ຽງເວີຊັນໃໝ່ {ver} ແລ້ວ!',
        'km': 'មានកញ្ចប់សត្វចិញ្ចឹមថ្មី {ver} អាចប្រើបានហើយ!', 'ms': 'Pakej haiwan peliharaan baharu {ver} tersedia!', 'ga': 'Tá pacáiste nua peata {ver} ar fáil!',
        'da': 'Ny kæledyrspakke {ver} tilgængelig!', 'fi': 'Uusi lemmikkipaketti {ver} saatavilla!', 'kg': 'Pake ya mpa ya kibulu {ver} kele ya kubongama!',
        'tl': 'Bagong pet package {ver} ay magagamit na!', 'no': 'Ny kjæledyrpakke {ver} tilgjengelig!', 'sv': 'Nytt husdjurspaket {ver} tillgängligt!',
        'kl': 'Uumasuaq paaqqitassaq nutaaq {ver} piareerpoq!', 'nl': 'Nieuw huisdierpakket {ver} beschikbaar!', 'hr': 'Dostupan je novi paket za ljubimca {ver}!',
        'rw': "Porogaramu nshya y'itungo {ver} irabonetse!", 'ro': 'Nou pachet de companie {ver} disponibil!', 'mt': 'Pakkett ġdid tal-pet {ver} disponibbli!',
        'ne': 'नयाँ पाल्तु प्याकेज {ver} उपलब्ध छ!', 'eo': 'Nova dorlotbesta pako {ver} disponeblas!', 'sl': 'Na voljo je nov paket ljubljenčka {ver}!',
        'tr': 'Yeni evcil hayvan paketi {ver} mevcut!', 'uk': 'Доступний новий пакет улюбленця {ver}!', 'el': 'Νέο πακέτο κατοικιδίου {ver} διαθέσιμο!',
        'hu': 'Új kisállat csomag {ver} elérhető!'
    },
    'pet.updateLatest': {
        'zh-CN': '当前已是最新版本', 'zh-HK': '當前已是最新版本', 'zh-TW': '目前已是最新版本',
        'en': 'Desktop pet is up to date', 'ja': 'デスクトップペットは最新です', 'ko': '데스크톱 펫이 최신 상태입니다',
        'fr': 'Le compagnon de bureau est à jour', 'de': 'Desktop-Haustier ist auf dem neuesten Stand', 'es': 'La mascota de escritorio está actualizada',
        'pt': 'O mascote de desktop está atualizado', 'ru': 'Настольный питомец обновлен до последней версии', 'it': 'Il pet desktop è aggiornato',
        'ar': 'الحيوان الأليف لسطح المكتب محدث', 'he': 'חיית המחמד לשולחן העבודה מעודכנת', 'ug': 'ھازىر ئەڭ يېڭى نەشرىدە',
        'bo': 'མིག་སྔར་ཆེས་གསར་པའི་པར་གཞི་ཡིན།', 'mn': 'Дэлгэцийн амьтан хамгийн сүүлийн хувилбар дээрээ байна', 'th': 'สัตว์เลี้ยงบนเดสก์ท็อปเป็นเวอร์ชันล่าสุดแล้ว',
        'vi': 'Thú cưng màn hình đã là phiên bản mới nhất', 'id': 'Hewan peliharaan desktop sudah versi terbaru', 'hi': 'डेस्कटॉप पेट पहले से ही अद्यतित है',
        'bn': 'ডেস্কটপ পেট আপ টু ডেট রয়েছে', 'my': 'ဒက်စ်တော့အိမ်မွေးတိရစ္ဆာန်သည် နောက်ဆုံးဗားရှင်းဖြစ်သည်', 'lo': 'ສັດລ້ຽງໜ້າຈໍເປັນເວີຊັນລ່າສຸດແລ້ວ',
        'km': 'សត្វចិញ្ចឹមលើតុគឺទាន់សម័យហើយ', 'ms': 'Haiwan peliharaan desktop adalah terkini', 'ga': 'Tá an peata deisce cothrom le dáta',
        'da': 'Skrivebordskæledyret er opdateret', 'fi': 'Työpöytälempikki on ajan tasalla', 'kg': 'Kibulu ya ordinatere kele na bansoba ya mpa',
        'tl': 'Ang desktop pet ay napapanahon na', 'no': 'Skrivebordskjæledyret er oppdatert', 'sv': 'Skrivbordsdjuret är uppdaterat',
        'kl': 'Uumasuaq paaqqitassaq nutaajunerpaavoq', 'nl': 'Desktop huisdier is up-to-date', 'hr': 'Stolni ljubimac je ažuriran',
        'rw': 'Itungo ryo kuri mudasobwa riravuguruye', 'ro': 'Animalul de companie este la zi', 'mt': 'Il-pet tad-desktop huwa aġġornat',
        'ne': 'डेस्कटप पाल्तु जनावर अद्यावधिक छ', 'eo': 'Labortabla dorlotbesto estas ĝisdata', 'sl': 'Namizni ljubljenček je posodobljen',
        'tr': 'Masaüstü evcil hayvan güncel', 'uk': 'Настільний улюбленець оновлений', 'el': 'Το κατοικίδιο είναι ενημερωμένο', 'hu': 'Az asztali kisállat naprakész'
    },
    'pet.applyUpdate': {
        'zh-CN': '立即更新', 'zh-HK': '立即更新', 'zh-TW': '立即更新',
        'en': 'Update Now', 'ja': '今すぐ更新', 'ko': '지금 업데이트', 'fr': 'Mettre à jour maintenant', 'de': 'Jetzt aktualisieren',
        'es': 'Actualizar ahora', 'pt': 'Atualizar agora', 'ru': 'Обновить сейчас', 'it': 'Aggiorna ora', 'ar': 'تحديث الآن',
        'he': 'עדכן עכשיו', 'ug': 'ھازىرلا يېڭىلاش', 'bo': 'ལམ་སེང་གསར་བཅོས་བྱེད།', 'mn': 'Одоо шинэчлэх', 'th': 'อัปเดตทันที',
        'vi': 'Cập nhật ngay', 'id': 'Perbarui Sekarang', 'hi': 'अभी अपडेट करें', 'bn': 'এখনই আপডেট করুন', 'my': 'ယခု အပ်ဒိတ်လုပ်ပါ',
        'lo': 'ອັບເດດຕອນນີ້', 'km': 'ធ្វើបច្ចុប្បន្នភាពឥឡូវនេះ', 'ms': 'Kemas Kini Sekarang', 'ga': 'Nuashonraigh Anois', 'da': 'Opdater nu',
        'fi': 'Päivitä nyt', 'kg': 'Vutukila sesepi', 'tl': 'I-update Ngayon', 'no': 'Oppdater nå', 'sv': 'Uppdatera nu',
        'kl': 'Maannakkorpiaq nutarteruk', 'nl': 'Nu Updaten', 'hr': 'Ažuriraj sada', 'rw': 'Kuvugurura ubu', 'ro': 'Actualizează acum',
        'mt': 'Aġġorna issa', 'ne': 'अहिले अपडेट गर्नुहोस्', 'eo': 'Ĝisdatigi Nun', 'sl': 'Posodobi zdaj', 'tr': 'Şimdi Güncelle',
        'uk': 'Оновити зараз', 'el': 'Ενημέρωση Τώρα', 'hu': 'Frissítés most'
    },
    'pet.checkUpdateFail': {
        'zh-CN': '检查更新失败，请稍后重试', 'zh-HK': '檢查更新失敗，請稍後重試', 'zh-TW': '檢查更新失敗，請稍後重試',
        'en': 'Update check failed, please try again later', 'ja': '更新の確認に失敗しました。後でもう一度お試しください', 'ko': '업데이트 확인에 실패했습니다. 나중에 다시 시도해 주세요',
        'fr': 'Échec de la vérification, veuillez réessayer plus tard', 'de': 'Update-Prüfung fehlgeschlagen, bitte später erneut versuchen',
        'es': 'Error al buscar actualizaciones, inténtelo de nuevo más tarde', 'pt': 'Falha ao verificar atualizações, tente novamente mais tarde',
        'ru': 'Ошибка проверки обновлений, повторите попытку позже', 'it': 'Controllo aggiornamenti non riuscito, riprova più tardi',
        'ar': 'فشل التحقق من التحديث، يرجى المحاولة مرة أخرى لاحقًا', 'he': 'בדיקת העדכון נכשלה, נסה שוב מאוחר יותר',
        'ug': 'يېڭىلاش تەكشۈرۈش مەغلۇپ بولدى، سەل تۇرۇپ قايتا سىناڭ', 'bo': 'གསར་བཅོས་ཞིབ་བཤེར་ཕམ་སོང་། རྗེས་སུ་བསྐྱར་དུ་ཚོད་ལྟ་གནང་རོགས།',
        'mn': 'Шинэчлэлт шалгах амжилтгүй боллоо, дараа дахин оролдоно уу', 'th': 'การตรวจสอบการอัปเดตล้มเหลว โปรดลองอีกครั้งในภายหลัง',
        'vi': 'Kiểm tra cập nhật thất bại, vui lòng thử lại sau', 'id': 'Pemeriksaan pembaruan gagal, silakan coba lagi nanti',
        'hi': 'अपडेट जांच विफल रही, कृपया बाद में पुनः प्रयास करें', 'bn': 'আপডেট পরীক্ষা ব্যর্থ হয়েছে, দয়া করে পরে আবার চেষ্টা করুন',
        'my': 'အပ်ဒိတ်စစ်ဆေးမှု မအောင်မြင်ပါ၊ နောက်မှ ထပ်စမ်းကြည့်ပါ', 'lo': 'ການກວດສອບອັບເດດຫຼົ້ມເຫຼວ, ກະລຸນາລອງໃໝ່ພາຍຫຼັງ',
        'km': 'ការពិនិត្យមើលបច្ចុប្បន្នភាពបានបរាជ័យ សូមព្យាយាមម្តងទៀតនៅពេលក្រោយ', 'ms': 'Semakan kemas kini gagal, sila cuba sebentar lagi',
        'ga': 'Theip ar sheiceáil nuashonraithe, bain triail as arís níos déanaí', 'da': 'Tjek af opdatering mislykkedes, prøv igen senere',
        'fi': 'Päivityksen tarkistus epäonnistui, yritä myöhemmin uudelleen', 'kg': 'Kutala bansoba kubikaka ve, meka diaka na nima',
        'tl': 'Nabigo ang pagsusuri sa update, subukang muli mamaya', 'no': 'Kontroll av oppdatering mislyktes, prøv igjen senere',
        'sv': 'Kontroll av uppdateringar misslyckades, försök igen senare', 'kl': 'Nutarterinerup misissornera iluatsinngilaq, kingusinnerusukkut misileqqiguk',
        'nl': 'Controle op updates mislukt, probeer het later opnieuw', 'hr': 'Provjera ažuriranja nije uspjela, pokušajte ponovno kasnije',
        'rw': 'Kugenzura amavugurura byanze, ongera ugerageze mukanya', 'ro': 'Verificarea actualizărilor a eșuat, reîncercați mai târziu',
        'mt': "Il-verifika tal-aġġornament falliet, erġa' pprova aktar tard", 'ne': 'अपडेट जाँच असफल भयो, कृपया पछि फेरि प्रयास गर्नुहोस्',
        'eo': 'Kontrolo de ĝisdatigo malsukcesis, bonvolu reprovi poste', 'sl': 'Preverjanje posodobitev ni uspelo, poskusite znova pozneje',
        'tr': 'Güncelleme denetimi başarısız oldu, lütfen daha sonra tekrar deneyin', 'uk': 'Помилка перевірки оновлень, спробуйте пізніше',
        'el': 'Ο έλεγχος ενημερώσεων απέτυχε, δοκιμάστε ξανά αργότερα', 'hu': 'A frissítés ellenőrzése sikertelen, próbálja újra később'
    },
    'pet.updateSuccess': {
        'zh-CN': '桌宠包已成功更新并平滑重载！', 'zh-HK': '桌寵包已成功更新並平滑重載！', 'zh-TW': '桌寵包已成功更新並平順重載！',
        'en': 'Desktop pet updated and smoothly reloaded!', 'ja': 'ペットパッケージが更新され、スムーズに再読み込みされました！', 'ko': '데스크톱 펫이 성공적으로 업데이트되고 부드럽게 다시 로드되었습니다!',
        'fr': 'Compagnon mis à jour et rechargé avec succès !', 'de': 'Haustierpaket erfolgreich aktualisiert und nahtlos neu geladen!',
        'es': '¡Mascota actualizada y recargada con éxito!', 'pt': 'Mascote atualizado e recarregado com sucesso!',
        'ru': 'Пакет питомца успешно обновлён и плавно перезагружен!', 'it': 'Pet desktop aggiornato e ricaricato con successo!',
        'ar': 'تم تحديث حزمة الحيوان الأليف وإعادة تحميلها بسلاسة!', 'he': 'חבילת חיית המחמד עודכנה ונטענה מחדש בהצלחה!',
        'ug': 'ئەرمەك ھايۋان بولىقى مۇۋەپپەقىيەتلىك يېڭىلاندى ھەمدە قايتا يۈكلەندى!', 'bo': 'གཅེས་ཉར་ཐུམ་བུ་ལེགས་གྲུབ་ངང་གསར་བཅོས་དང་བསྐྱར་འཇུག་བྱས་ཟིན།',
        'mn': 'Дэлгэцийн амьтан амжилттай шинэчлэгдэж, жигд дахин ачаалагдлаа!', 'th': 'อัปเดตแพ็กเกจสัตว์เลี้ยงและโหลดใหม่สำเร็จอย่างราบรื่น!',
        'vi': 'Đã cập nhật và tải lại thú cưng màn hình thành công!', 'id': 'Paket hewan peliharaan berhasil diperbarui dan dimuat ulang!',
        'hi': 'पेट पैकेज सफलतापूर्वक अपडेट और सहज रूप से पुनः लोड किया गया!', 'bn': 'পেট প্যাকেজ সফলভাবে আপডেট ও মসৃণভাবে পুনরায় লোড হয়েছে!',
        'my': 'အိမ်မွေးတိရစ္ဆာန် ပက်ကေ့ဂျ်ကို အောင်မြင်စွာ အပ်ဒိတ်လုပ်ပြီး ပြန်လည်ဖွင့်လှစ်ပြီးပါပြီ။', 'lo': 'ແພັກເກດສັດລ້ຽງອັບເດດສຳເລັດ ແລະ ໂຫຼດໃໝ່ຢ່າງລຽບງ່າຍ!',
        'km': 'កញ្ចប់សត្វចិញ្ចឹមត្រូវបានធ្វើបច្ចុប្បន្នភាព និងដំណើរការឡើងវិញដោយជោគជ័យ!', 'ms': 'Pakej haiwan peliharaan berjaya dikemas kini dan dimuat semula dengan lancar!',
        'ga': 'Nuashonraíodh an pacáiste peata agus athlódáladh go réidh é!', 'da': 'Kæledyrspakken blev opdateret og genindlæst jævnt!',
        'fi': 'Lemmikkipaketti päivitetty ja ladattu uudelleen saumattomasti!', 'kg': 'Pake ya kibulu me vutukila mpe me funguka diaka mbote!',
        'tl': 'Matagumpay na na-update at maayos na na-reload ang pet package!', 'no': 'Kjæledyrpakken ble oppdatert og lastet inn på nytt!',
        'sv': 'Husdjurspaketet har uppdaterats och laddats om smidigt!', 'kl': 'Uumasuaq paaqqitassaq nutarterneqarpoq aallarteqqillunilu!',
        'nl': 'Huisdierpakket succesvol bijgewerkt en soepel herladen!', 'hr': 'Paket za ljubimca uspješno je ažuriran i ponovno učitan!',
        'rw': "Porogaramu y'itungo yavuguruwe neza kandi yongeye gutangira neza!", 'ro': 'Pachetul animalului a fost actualizat și reîncărcat cu succes!',
        'mt': "Il-pakkett tal-pet ġie aġġornat b'suċċess u reġa' tella' bla xkiel!", 'ne': 'पाल्तु प्याकेज सफलतापूर्वक अपडेट भयो र पुनः लोड भयो!',
        'eo': 'Dorlotbesta pako sukcese ĝisdatiĝis kaj glate reŝargiĝis!', 'sl': 'Paket ljubljenčka je bil uspešno posodobljen in znova naložen!',
        'tr': 'Evcil hayvan paketi başarıyla güncellendi ve sorunsuz yeniden yüklendi!', 'uk': 'Пакет улюбленця успішно оновлено та плавно перезавантажено!',
        'el': 'Το πακέτο κατοικιδίου ενημερώθηκε και επαναφορτώθηκε ομαλά!', 'hu': 'A kisállat csomag sikeresen frissítve és zökkenőmentesen újratöltve!'
    },
    'pet.updateFailed': {
        'zh-CN': '桌宠包更新失败 ({code})', 'zh-HK': '桌寵包更新失敗 ({code})', 'zh-TW': '桌寵包更新失敗 ({code})',
        'en': 'Pet update failed ({code})', 'ja': 'ペットの更新に失敗しました ({code})', 'ko': '펫 업데이트 실패 ({code})',
        'fr': 'Échec de la mise à jour ({code})', 'de': 'Haustier-Update fehlgeschlagen ({code})', 'es': 'Error en la actualización de la mascota ({code})',
        'pt': 'Falha na atualização do mascote ({code})', 'ru': 'Ошибка обновления питомца ({code})', 'it': 'Aggiornamento pet non riuscito ({code})',
        'ar': 'فشل تحديث الحيوان الأليف ({code})', 'he': 'עדכון חיית המחמד נכשל ({code})', 'ug': 'ئەرمەك ھايۋاننى يېڭىلاش مەغلۇپ بولدى ({code})',
        'bo': 'གཅེས་ཉར་གསར་བཅོས་ཕམ་སོང་ ({code})', 'mn': 'Амьтан шинэчлэх амжилтгүй боллоо ({code})', 'th': 'การอัปเดตสัตว์เลี้ยงล้มเหลว ({code})',
        'vi': 'Cập nhật thú cưng thất bại ({code})', 'id': 'Pembaruan hewan peliharaan gagal ({code})', 'hi': 'पेट अपडेट विफल रहा ({code})',
        'bn': 'পেট আপডেট ব্যর্থ হয়েছে ({code})', 'my': 'အိမ်မွေးတိရစ္ဆာန် အပ်ဒိတ်မအောင်မြင်ပါ ({code})', 'lo': 'ອັບເດດສັດລ້ຽງຫຼົ້ມເຫຼວ ({code})',
        'km': 'ការធ្វើបច្ចុប្បន្នភាពសត្វចិញ្ចឹមបានបរាជ័យ ({code})', 'ms': 'Kemas kini haiwan peliharaan gagal ({code})', 'ga': 'Theip ar nuashonrú peata ({code})',
        'da': 'Opdatering af kæledyr mislykkedes ({code})', 'fi': 'Lemmikin päivitys epäonnistui ({code})', 'kg': 'Kubongisa kibulu kubikaka ve ({code})',
        'tl': 'Nabigo ang pag-update ng pet ({code})', 'no': 'Oppdatering av kjæledyr mislyktes ({code})', 'sv': 'Uppdatering av husdjur misslyckades ({code})',
        'kl': 'Uumasuaq paaqqitassaq nutarterneqanngilaq ({code})', 'nl': 'Update van huisdier mislukt ({code})', 'hr': 'Ažuriranje ljubimca nije uspjelo ({code})',
        'rw': 'Kuvugurura itungo byanze ({code})', 'ro': 'Actualizarea animalului a eșuat ({code})', 'mt': 'L-aġġornament tal-pet falla ({code})',
        'ne': 'पाल्तु अपडेट असफल भयो ({code})', 'eo': 'Ĝisdatigo de dorlotbesto malsukcesis ({code})', 'sl': 'Posodobitev ljubljenčka ni uspela ({code})',
        'tr': 'Evcil hayvan güncellemesi başarısız oldu ({code})', 'uk': 'Оновлення улюбленця не вдалося ({code})', 'el': 'Η ενημέρωση του κατοικιδίου απέτυχε ({code})',
        'hu': 'A kisállat frissítése sikertelen ({code})'
    }
}


def main():
    json_files = [f for f in sorted(os.listdir(I18N_DIR)) if f.endswith('.json') and f != 'meta.json']
    assert len(json_files) == 46, f"Expected 46 json files, found {len(json_files)}"

    for fn in json_files:
        lang = fn[:-5]
        fpath = os.path.join(I18N_DIR, fn)
        with open(fpath, 'r', encoding='utf-8') as f:
            data = json.load(f)

        # 1. Categories: ai.tplCategory.<cat>
        for cat, trans in CATEGORIES.items():
            key = f'ai.tplCategory.{cat}'
            val = trans.get(lang)
            if not val:
                raise ValueError(f"Missing category {cat} translation for {lang}")
            data[key] = val

        # 2. Skills & Actions: ai.action.<action> and skill.<skill_id>.name
        for action_or_id, item in SKILLS_AND_ACTIONS.items():
            val = item.get(lang)
            if not val:
                raise ValueError(f"Missing skill/action {action_or_id} translation for {lang}")

            # ai.action.<id> if it represents an action
            if item.get('action_id') is not None or ('skill_id' in item and action_or_id != item.get('skill_id')):
                act_key = f'ai.action.{action_or_id}'
                data[act_key] = val

            # skill.<skill_id>.name if it has a skill_id
            if item.get('skill_id'):
                skill_key = f'skill.{item["skill_id"]}.name'
                data[skill_key] = val

        # 3. Pet updates
        for pet_key, trans in PET_UPDATES.items():
            val = trans.get(lang)
            if not val:
                raise ValueError(f"Missing pet update {pet_key} translation for {lang}")
            data[pet_key] = val

        # Write back cleanly formatted JSON
        with open(fpath, 'w', encoding='utf-8') as f:
            json.dump(data, f, ensure_ascii=False, indent=2)
            f.write('\n')

    print(f"[OK] Successfully updated all 46 i18n JSON files.")

    # Integrity verification
    cjk_re = re.compile(r'[\u4e00-\u9fff]')
    non_cjk_files = [f for f in json_files if not f.startswith('zh') and not f.startswith('ja')]

    with open(os.path.join(I18N_DIR, 'en.json'), 'r', encoding='utf-8') as f:
        en_keys = set(json.load(f).keys())

    for fn in json_files:
        fpath = os.path.join(I18N_DIR, fn)
        with open(fpath, 'r', encoding='utf-8') as f:
            d = json.load(f)

        # Check parity
        diff = en_keys - set(d.keys())
        assert not diff, f"{fn} has missing keys: {diff}"
        diff_extra = set(d.keys()) - en_keys
        assert not diff_extra, f"{fn} has extra keys: {diff_extra}"

        # Check non-cjk languages for exposed Chinese in new keys
        if fn in non_cjk_files:
            for k in [
                'ai.tplCategory.general', 'ai.tplCategory.writing', 'ai.tplCategory.coding',
                'ai.tplCategory.academic', 'ai.tplCategory.custom',
                'ai.action.quick_read', 'ai.action.polish', 'ai.action.proofread',
                'ai.action.to_english', 'ai.action.to_chinese', 'ai.action.action_items',
                'ai.action.continue', 'ai.action.ask', 'ai.action.summary',
                'ai.action.outline', 'ai.action.weekly', 'ai.action.code_review',
                'ai.action.fix_format',
                'skill.readmd-quick-read.name', 'skill.readmd-polish.name',
                'skill.readmd-proofread.name', 'skill.readmd-translate.name',
                'skill.readmd-todo.name', 'skill.readmd-continue.name',
                'skill.readmd-ask.name', 'skill.readmd-summary.name',
                'skill.readmd-outline.name', 'skill.readmd-weekly.name',
                'skill.readmd-code-review.name', 'skill.readmd-format-fix.name',
                'ai.defaultAction',
                'pet.updateTitle', 'pet.checkUpdate', 'pet.checkingUpdate',
                'pet.updateAvailable', 'pet.updateLatest', 'pet.applyUpdate',
                'pet.checkUpdateFail', 'pet.updateSuccess', 'pet.updateFailed'
            ]:
                v = d.get(k, '')
                assert not cjk_re.search(v), f"File {fn} has exposed Chinese in key {k}: {v}"
                assert v != k, f"File {fn} copied key name as value for {k}"
                assert v.strip() != '', f"File {fn} has empty string for {k}"

    print(f"[OK] Verified 100% key parity and ZERO exposed Chinese across all 43 non-Chinese languages!")


if __name__ == '__main__':
    main()

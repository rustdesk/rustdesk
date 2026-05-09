import 'package:flutter/material.dart';
import '../../common.dart';

class ImportRustdeskGuideModal extends StatefulWidget {
  const ImportRustdeskGuideModal({Key? key}) : super(key: key);

  @override
  State<ImportRustdeskGuideModal> createState() =>
      _ImportRustdeskGuideModalState();
}

class _ImportRustdeskGuideModalState
    extends State<ImportRustdeskGuideModal> {
  final PageController _controller = PageController();
  int _currentPage = 0;

  final List<_GuideSlide> _slides = const [
    _GuideSlide(
      icon: Icons.swap_horiz,
      title: 'Import from RustDesk',
      body:
          'This will import your saved connections and server settings from the RustDesk app on this device.\n\nFollow the steps below to export your data first.',
    ),
    _GuideSlide(
      icon: Icons.folder_open,
      title: 'Open Files App',
      body:
          'Open the Files app on your iPhone. You can find it on your Home Screen or in your App Library.',
    ),
    _GuideSlide(
      icon: Icons.phone_iphone,
      title: 'Find the RustDesk Folder',
      body:
          'Navigate to:\n\nOn My iPhone → RustDesk → data\n\nThis folder contains your saved connections and settings.',
    ),
    _GuideSlide(
      icon: Icons.share,
      title: 'Save the Folder',
      body:
          'Long-press the "data" folder, then tap Share → Save to Files.\n\nSave it somewhere easy to find, such as iCloud Drive or On My iPhone.',
    ),
    _GuideSlide(
      icon: Icons.check_circle_outline,
      title: 'You\'re Ready',
      body:
          'Go back to Tabby Settings and tap "Import from RustDesk" to select the folder you just saved.',
    ),
  ];

  @override
  void dispose() {
    _controller.dispose();
    super.dispose();
  }

  void _next() {
    if (_currentPage < _slides.length - 1) {
      _controller.nextPage(
          duration: const Duration(milliseconds: 300),
          curve: Curves.easeInOut);
    } else {
      Navigator.of(context).pop();
    }
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(
        title: Text(translate('How to Export from RustDesk')),
        actions: [
          TextButton(
            onPressed: () => Navigator.of(context).pop(),
            child: Text(translate('Skip'),
                style: const TextStyle(color: Colors.white)),
          ),
        ],
      ),
      body: Column(
        children: [
          Expanded(
            child: PageView.builder(
              controller: _controller,
              itemCount: _slides.length,
              onPageChanged: (i) => setState(() => _currentPage = i),
              itemBuilder: (context, i) =>
                  _SlideView(slide: _slides[i]),
            ),
          ),
          _DotsIndicator(
              count: _slides.length, current: _currentPage),
          const SizedBox(height: 16),
          Padding(
            padding: const EdgeInsets.symmetric(horizontal: 24, vertical: 16),
            child: SizedBox(
              width: double.infinity,
              child: ElevatedButton(
                onPressed: _next,
                child: Text(
                  _currentPage < _slides.length - 1
                      ? translate('Next')
                      : translate('Done'),
                ),
              ),
            ),
          ),
        ],
      ),
    );
  }
}

class _GuideSlide {
  final IconData icon;
  final String title;
  final String body;
  const _GuideSlide(
      {required this.icon, required this.title, required this.body});
}

class _SlideView extends StatelessWidget {
  final _GuideSlide slide;
  const _SlideView({required this.slide});

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.symmetric(horizontal: 32, vertical: 24),
      child: Column(
        mainAxisAlignment: MainAxisAlignment.center,
        children: [
          Icon(slide.icon, size: 72, color: Theme.of(context).primaryColor),
          const SizedBox(height: 32),
          Text(slide.title,
              style: Theme.of(context).textTheme.headlineSmall,
              textAlign: TextAlign.center),
          const SizedBox(height: 20),
          Text(slide.body,
              style: Theme.of(context).textTheme.bodyLarge,
              textAlign: TextAlign.center),
        ],
      ),
    );
  }
}

class _DotsIndicator extends StatelessWidget {
  final int count;
  final int current;
  const _DotsIndicator({required this.count, required this.current});

  @override
  Widget build(BuildContext context) {
    return Row(
      mainAxisAlignment: MainAxisAlignment.center,
      children: List.generate(
        count,
        (i) => AnimatedContainer(
          duration: const Duration(milliseconds: 200),
          margin: const EdgeInsets.symmetric(horizontal: 4),
          width: i == current ? 16 : 8,
          height: 8,
          decoration: BoxDecoration(
            color: i == current
                ? Theme.of(context).primaryColor
                : Colors.grey.shade400,
            borderRadius: BorderRadius.circular(4),
          ),
        ),
      ),
    );
  }
}

void showImportRustdeskGuide(BuildContext context) {
  Navigator.of(context).push(MaterialPageRoute(
    builder: (_) => const ImportRustdeskGuideModal(),
    fullscreenDialog: true,
  ));
}

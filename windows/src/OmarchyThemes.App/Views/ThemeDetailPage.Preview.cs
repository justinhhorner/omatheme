using System.ComponentModel;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Input;
using Microsoft.UI.Xaml.Media.Imaging;
using OmarchyThemes.App.Helpers;
using OmarchyThemes.App.ViewModels;
using Windows.System;

namespace OmarchyThemes.App.Views;

/// <summary>The large wallpaper preview overlay: opening, keys, loading state and focus.</summary>
public sealed partial class ThemeDetailPage
{
    /// <summary>What had focus before the preview opened (the Preview button or a thumbnail).</summary>
    private Control? _previewReturnFocus;

    private void OnViewModelPropertyChanged(object? sender, PropertyChangedEventArgs e)
    {
        if (e.PropertyName == nameof(ThemeDetailViewModel.PreviewItem))
            UpdatePreviewLoadingState();
        else if (e.PropertyName == nameof(ThemeDetailViewModel.IsPreviewOpen))
            OnPreviewOpenChanged();
    }

    private void OpenPreview(WallpaperItemViewModel? item, Control? returnFocus)
    {
        _previewReturnFocus = returnFocus;
        ViewModel.OpenPreview(item);
    }

    private void OnPreviewOpenChanged()
    {
        if (ViewModel.IsPreviewOpen)
        {
            UpdatePreviewLoadingState();
            // Let the overlay become visible before moving focus into it.
            DispatcherQueue.TryEnqueue(() => PreviewNextButton.Focus(FocusState.Programmatic));
            return;
        }

        // Back to the thumbnail that was previewed last (it may differ from where the preview was opened).
        var target = ViewModel.PreviewItem is { } item && _previewReturnFocus != PreviewButton
            ? WallpaperGrid.ContainerFromItem(item) as Control
            : null;
        (target ?? _previewReturnFocus ?? PreviewButton).Focus(FocusState.Programmatic);
    }

    private void UpdatePreviewLoadingState()
    {
        PreviewFailedText.Visibility = Visibility.Collapsed;
        // A remote image that hasn't been decoded yet shows a spinner; cached or local ones are instant.
        var loaded = ViewModel.PreviewItem?.Preview is BitmapImage { PixelWidth: > 0 };
        PreviewSpinner.IsActive = ViewModel.IsPreviewOpen && !loaded;
    }

    private void PreviewImage_ImageOpened(object sender, RoutedEventArgs e) => PreviewSpinner.IsActive = false;

    private void PreviewImage_ImageFailed(object sender, ExceptionRoutedEventArgs e)
    {
        PreviewSpinner.IsActive = false;
        PreviewFailedText.Visibility = Visibility.Visible;
    }

    private void PreviewButton_Click(object sender, RoutedEventArgs e) => OpenPreview(null, PreviewButton);

    /// <summary>A thumbnail's hover button or its context menu item.</summary>
    private void PreviewItem_Click(object sender, RoutedEventArgs e)
    {
        if (Ui.ItemOf<WallpaperItemViewModel>(sender) is { } item)
            OpenPreview(item, WallpaperGrid.ContainerFromItem(item) as Control);
    }

    private void WallpaperItem_DoubleTapped(object sender, DoubleTappedRoutedEventArgs e)
    {
        if (Ui.ItemOf<WallpaperItemViewModel>(sender) is { } item)
        {
            OpenPreview(item, WallpaperGrid.ContainerFromItem(item) as Control);
            e.Handled = true;
        }
    }

    private void WallpaperItem_PointerEntered(object sender, PointerRoutedEventArgs e) => SetExpandButtonOpacity(sender, 1);

    private void WallpaperItem_PointerExited(object sender, PointerRoutedEventArgs e) => SetExpandButtonOpacity(sender, 0);

    private static void SetExpandButtonOpacity(object sender, double opacity)
    {
        if ((sender as FrameworkElement)?.FindName("ExpandButton") is UIElement button)
            button.Opacity = opacity;
    }

    private void PreviewOverlay_KeyDown(object sender, KeyRoutedEventArgs e)
    {
        switch (e.Key)
        {
            case VirtualKey.Left:
                ViewModel.MovePreview(-1);
                break;
            case VirtualKey.Right:
                ViewModel.MovePreview(+1);
                break;
            case VirtualKey.Escape:
                ViewModel.ClosePreview();
                break;
            default:
                return;
        }
        e.Handled = true;
    }

    /// <summary>A click anywhere outside the control bar closes the preview.</summary>
    private void PreviewOverlay_Tapped(object sender, TappedRoutedEventArgs e) => ViewModel.ClosePreview();

    private void PreviewBar_Tapped(object sender, TappedRoutedEventArgs e) => e.Handled = true;
}
